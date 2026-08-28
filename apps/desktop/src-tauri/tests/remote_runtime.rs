#![cfg(feature = "integration-test-support")]

use std::collections::HashMap;
use std::fs;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use gameboy_desktop::integration_test_support::{
    CoreFactory, KEYBOARD_INPUT_SOURCE, REMOTE_INPUT_SOURCE, RemotePhase, RemoteRuntimeHarness,
    RuntimeButton, RuntimeCore, RuntimePhase,
};
use gb_core::{
    AudioBatch, BatteryState, Button, CartridgeMetadata, CompatibilityMode, CoreError, Frame,
    InputSourceId, JoypadState, MapperKind, RunOutcome,
};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type ClientSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
const SOCKET_TIMEOUT: Duration = Duration::from_secs(2);

struct TestAssets {
    root: PathBuf,
}

impl TestAssets {
    fn new() -> Self {
        let root = fixture_path("controller-assets");
        fs::create_dir_all(&root).expect("create controller asset root");
        fs::write(
            root.join("index.html"),
            "<!doctype html><title>Controller fixture</title>",
        )
        .expect("write controller fixture");
        Self { root }
    }
}

impl Drop for TestAssets {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Default)]
struct RecordingState {
    inputs: HashMap<InputSourceId, JoypadState>,
    applications: HashMap<InputSourceId, usize>,
    clears: Vec<InputSourceId>,
}

impl RecordingState {
    fn input(&self, source: InputSourceId) -> Option<JoypadState> {
        self.inputs.get(&source).copied()
    }

    fn clear_count(&self, source: InputSourceId) -> usize {
        self.clears
            .iter()
            .filter(|cleared| **cleared == source)
            .count()
    }

    fn application_count(&self, source: InputSourceId) -> usize {
        self.applications.get(&source).copied().unwrap_or_default()
    }
}

#[derive(Default)]
struct RecordingCoreFactory {
    state: Arc<Mutex<RecordingState>>,
}

impl CoreFactory for RecordingCoreFactory {
    fn create(&self) -> Box<dyn RuntimeCore> {
        Box::new(RecordingCore {
            state: Arc::clone(&self.state),
            loaded: false,
        })
    }
}

struct RecordingCore {
    state: Arc<Mutex<RecordingState>>,
    loaded: bool,
}

impl gb_core::EmulatorCore for RecordingCore {
    fn load_rom(
        &mut self,
        _rom: &[u8],
        _persisted: Option<&BatteryState>,
    ) -> Result<CartridgeMetadata, CoreError> {
        self.loaded = true;
        Ok(CartridgeMetadata {
            title: "Remote E2E".into(),
            rom_identity: "remote-e2e".into(),
            mapper: MapperKind::RomOnly,
            compatibility: CompatibilityMode::Dmg,
            ram_size_bytes: 0,
            has_battery: false,
        })
    }

    fn reset(&mut self) -> Result<(), CoreError> {
        self.loaded.then_some(()).ok_or(CoreError::NotLoaded)
    }

    fn run_cycles(&mut self, cycle_budget: u32) -> Result<RunOutcome, CoreError> {
        self.loaded
            .then(|| RunOutcome::idle(cycle_budget))
            .ok_or(CoreError::NotLoaded)
    }

    fn set_input(&mut self, source: InputSourceId, state: JoypadState) {
        let mut recording = self.state.lock().expect("recording state");
        *recording.applications.entry(source).or_default() += 1;
        recording.inputs.insert(source, state);
    }

    fn clear_input_source(&mut self, source: InputSourceId) {
        let mut state = self.state.lock().expect("recording state");
        state.inputs.remove(&source);
        state.clears.push(source);
    }

    fn take_frame(&mut self) -> Option<Frame> {
        None
    }

    fn drain_audio(&mut self) -> AudioBatch {
        AudioBatch::empty(NonZeroU32::new(48_000).expect("non-zero sample rate"))
    }

    fn battery_state(&self) -> Option<BatteryState> {
        None
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)]
async fn real_websocket_protocol_drives_runtime_reconnects_and_stays_bounded() {
    let assets = TestAssets::new();
    let first_rom = synthetic_rom("first");
    let replacement_rom = synthetic_rom("replacement");
    let factory = Arc::new(RecordingCoreFactory::default());
    let recording = Arc::clone(&factory.state);
    let harness = RemoteRuntimeHarness::new(factory, assets.root.clone());

    harness
        .open_rom(first_rom.clone())
        .expect("first ROM loads");
    harness.start_runtime().expect("runtime starts");
    harness
        .set_keyboard_input(vec![RuntimeButton::A])
        .expect("keyboard A");

    let waiting = harness.start_remote().expect("loopback session starts");
    assert_eq!(waiting.phase, RemotePhase::Waiting);
    let pairing_url = waiting.pairing_url.expect("pairing URL");
    let token = token_from_pairing_url(&pairing_url).to_owned();

    let mut invalid = connect_controller(&pairing_url).await;
    send_json(
        &mut invalid,
        json!({"type":"hello","version":"v1","token":"invalid"}),
    )
    .await;
    assert_eq!(
        next_json(&mut invalid).await,
        json!({"type":"rejected","reason":"invalid-token"})
    );

    let mut active = connect_controller(&pairing_url).await;
    authenticate(&mut active, &token).await;
    assert_eq!(
        harness.remote_snapshot().expect("connected snapshot").phase,
        RemotePhase::Connected
    );

    let mut sequence = 0_u64;
    send_json(
        &mut active,
        json!({"type":"state-sync","buttons":["left","a"],"sequence":sequence}),
    )
    .await;
    sequence += 1;
    ping_barrier(&mut active, sequence).await;
    sequence += 1;
    wait_for_input(&recording, REMOTE_INPUT_SOURCE, |state| {
        state.is_pressed(Button::Left) && state.is_pressed(Button::A)
    })
    .await;
    assert!(
        recording
            .lock()
            .expect("recording state")
            .input(KEYBOARD_INPUT_SOURCE)
            .expect("keyboard state")
            .is_pressed(Button::A)
    );

    let mut second = connect_controller(&pairing_url).await;
    send_json(
        &mut second,
        json!({"type":"hello","version":"v1","token":token}),
    )
    .await;
    assert_eq!(
        next_json(&mut second).await,
        json!({"type":"rejected","reason":"controller-already-connected"})
    );
    send_json(
        &mut active,
        json!({"type":"button-down","button":"b","sequence":sequence}),
    )
    .await;
    sequence += 1;
    ping_barrier(&mut active, sequence).await;
    sequence += 1;
    wait_for_input(&recording, REMOTE_INPUT_SOURCE, |state| {
        state.is_pressed(Button::B)
    })
    .await;

    harness.pause_runtime().expect("runtime pauses");
    assert_remote_connected(&harness);
    harness.start_runtime().expect("runtime resumes");
    assert_remote_connected(&harness);
    harness.restart_runtime().expect("runtime restarts");
    assert_remote_connected(&harness);
    harness
        .open_rom(replacement_rom.clone())
        .expect("replacement ROM loads");
    assert_remote_connected(&harness);
    harness.start_runtime().expect("replacement starts");
    harness
        .set_keyboard_input(vec![RuntimeButton::A])
        .expect("keyboard A restored after replacement");

    let remote_applications_before = recording
        .lock()
        .expect("recording state")
        .application_count(REMOTE_INPUT_SOURCE);
    for transition in 0..200_u64 {
        let message_type = if transition % 2 == 0 {
            "button-down"
        } else {
            "button-up"
        };
        send_json(
            &mut active,
            json!({
                "type":message_type,
                "button":"select",
                "sequence":sequence
            }),
        )
        .await;
        sequence += 1;
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    ping_barrier(&mut active, sequence).await;
    let remote_applications_after = recording
        .lock()
        .expect("recording state")
        .application_count(REMOTE_INPUT_SOURCE);
    assert_eq!(
        remote_applications_after.checked_sub(remote_applications_before),
        Some(200),
        "each loopback transition reaches the recording core exactly once"
    );
    let latency = harness
        .remote_snapshot()
        .expect("latency snapshot")
        .latency
        .expect("latency is recorded");
    assert_eq!(latency.samples, 128);
    assert!(
        latency.p95_ms < 100,
        "loopback p95 was {} ms",
        latency.p95_ms
    );

    drop(active);
    wait_for_remote_phase(&harness, RemotePhase::Waiting).await;
    wait_for_missing_input(&recording, REMOTE_INPUT_SOURCE).await;
    {
        let state = recording.lock().expect("recording state");
        assert!(
            state
                .input(KEYBOARD_INPUT_SOURCE)
                .expect("keyboard remains active")
                .is_pressed(Button::A)
        );
        assert_eq!(state.clear_count(REMOTE_INPUT_SOURCE), 1);
    }
    assert_eq!(
        harness.runtime_snapshot().expect("runtime snapshot").phase,
        RuntimePhase::Running
    );

    let mut reconnected = connect_controller(&pairing_url).await;
    authenticate(&mut reconnected, token_from_pairing_url(&pairing_url)).await;
    send_json(
        &mut reconnected,
        json!({"type":"state-sync","buttons":["down","b"],"sequence":0}),
    )
    .await;
    ping_barrier(&mut reconnected, 1).await;
    wait_for_input(&recording, REMOTE_INPUT_SOURCE, |state| {
        state.is_pressed(Button::Down)
            && state.is_pressed(Button::B)
            && !state.is_pressed(Button::Left)
            && !state.is_pressed(Button::A)
    })
    .await;

    let off = harness.end_remote().expect("explicit session end");
    assert_eq!(off.phase, RemotePhase::Off);
    wait_for_missing_input(&recording, REMOTE_INPUT_SOURCE).await;
    {
        let state = recording.lock().expect("recording state");
        assert!(
            state
                .input(KEYBOARD_INPUT_SOURCE)
                .expect("keyboard survives explicit end")
                .is_pressed(Button::A)
        );
        assert_eq!(state.clear_count(REMOTE_INPUT_SOURCE), 2);
    }
    let old_token_connection = tokio::time::timeout(
        Duration::from_secs(1),
        tokio_tungstenite::connect_async(websocket_url(&pairing_url)),
    )
    .await
    .expect("old URL fails promptly");
    assert!(
        old_token_connection.is_err(),
        "ended session URL remained valid"
    );

    harness.shutdown_runtime().expect("runtime shutdown");
    fs::remove_file(first_rom).expect("remove first ROM");
    fs::remove_file(replacement_rom).expect("remove replacement ROM");
}

fn assert_remote_connected(harness: &RemoteRuntimeHarness) {
    assert_eq!(
        harness.remote_snapshot().expect("remote snapshot").phase,
        RemotePhase::Connected
    );
}

async fn connect_controller(pairing_url: &str) -> ClientSocket {
    tokio::time::timeout(
        SOCKET_TIMEOUT,
        tokio_tungstenite::connect_async(websocket_url(pairing_url)),
    )
    .await
    .expect("controller websocket connection completes before timeout")
    .expect("connect controller websocket")
    .0
}

async fn authenticate(socket: &mut ClientSocket, token: &str) {
    send_json(socket, json!({"type":"hello","version":"v1","token":token})).await;
    assert_eq!(next_json(socket).await["type"], "welcome");
}

async fn ping_barrier(socket: &mut ClientSocket, sequence: u64) {
    send_json(socket, json!({"type":"ping","sequence":sequence})).await;
    assert_eq!(
        next_json(socket).await,
        json!({"type":"pong","sequence":sequence})
    );
}

async fn send_json(socket: &mut ClientSocket, value: Value) {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .expect("send protocol JSON");
}

async fn next_json(socket: &mut ClientSocket) -> Value {
    let message = tokio::time::timeout(SOCKET_TIMEOUT, socket.next())
        .await
        .expect("server response arrives before timeout")
        .expect("server response")
        .expect("valid server response");
    let Message::Text(text) = message else {
        panic!("expected text response, got {message:?}");
    };
    serde_json::from_str(&text).expect("server response JSON")
}

async fn wait_for_remote_phase(harness: &RemoteRuntimeHarness, expected: RemotePhase) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if harness
            .remote_snapshot()
            .is_ok_and(|snapshot| snapshot.phase == expected)
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "remote phase becomes {expected:?}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

async fn wait_for_input(
    recording: &Mutex<RecordingState>,
    source: InputSourceId,
    predicate: impl Fn(JoypadState) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if recording
            .lock()
            .expect("recording state")
            .input(source)
            .is_some_and(&predicate)
        {
            return;
        }
        assert!(Instant::now() < deadline, "input source {source:?} updates");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

async fn wait_for_missing_input(recording: &Mutex<RecordingState>, source: InputSourceId) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if recording
            .lock()
            .expect("recording state")
            .input(source)
            .is_none()
        {
            return;
        }
        assert!(Instant::now() < deadline, "input source {source:?} clears");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn token_from_pairing_url(pairing_url: &str) -> &str {
    pairing_url
        .split_once("?token=")
        .map(|(_, token)| token)
        .expect("pairing token")
}

fn websocket_url(pairing_url: &str) -> String {
    let address = pairing_url
        .strip_prefix("http://")
        .and_then(|url| url.split('/').next())
        .expect("pairing socket address");
    format!("ws://{address}/controller")
}

fn synthetic_rom(label: &str) -> PathBuf {
    let path = fixture_path(&format!("{label}.gb"));
    fs::write(&path, b"PED-39 remote runtime integration ROM").expect("write synthetic ROM");
    path
}

fn fixture_path(label: &str) -> PathBuf {
    let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "ped-39-remote-runtime-{}-{id}-{label}",
        std::process::id()
    ))
}
