use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use gb_core::{Button, CartridgeMetadata, EmulatorCore, Frame, InputSourceId, JoypadState};

use super::contracts::{
    RomSummary, RuntimeButton, RuntimeError, RuntimeErrorCode, RuntimeEvent, RuntimePhase,
    RuntimeResult, RuntimeSnapshot, encode_frame_packet,
};

const COMMAND_QUEUE_CAPACITY: usize = 32;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const FRAME_INTERVAL: Duration = Duration::from_micros(16_743);
const FRAME_CYCLE_BUDGET: u32 = 70_224;
const KEYBOARD_INPUT_SOURCE: InputSourceId = InputSourceId::new(1);

pub trait RuntimeCore: EmulatorCore + Send {}
impl<T: EmulatorCore + Send> RuntimeCore for T {}

pub trait CoreFactory: Send + Sync {
    fn create(&self) -> Box<dyn RuntimeCore>;
}

pub trait RuntimeObserver: Send + Sync {
    fn publish_control(&self, event: RuntimeEvent) -> RuntimeResult<()>;
    fn publish_frame(&self, packet: Vec<u8>) -> RuntimeResult<()>;
}

#[derive(Default)]
struct FrameDelivery {
    in_flight_sequence: Option<u64>,
    latest_pending: Option<Frame>,
}

impl FrameDelivery {
    fn clear(&mut self) {
        self.in_flight_sequence = None;
        self.latest_pending = None;
    }

    #[cfg(test)]
    fn buffered_frame_count(&self) -> usize {
        usize::from(self.in_flight_sequence.is_some()) + usize::from(self.latest_pending.is_some())
    }
}

#[derive(Default)]
struct RuntimeModel {
    snapshot: RuntimeSnapshot,
    core_loaded: bool,
}

impl RuntimeModel {
    fn snapshot(&self) -> RuntimeSnapshot {
        self.snapshot.clone()
    }

    fn begin_load(&mut self) {
        self.core_loaded = false;
        self.snapshot = RuntimeSnapshot {
            phase: RuntimePhase::Loading,
            rom: None,
            error: None,
        };
    }

    fn finish_load(&mut self, metadata: CartridgeMetadata, file_name: String) {
        self.core_loaded = true;
        self.snapshot = RuntimeSnapshot {
            phase: RuntimePhase::Paused,
            rom: Some(RomSummary::from_metadata(metadata, file_name)),
            error: None,
        };
    }

    fn fail_load(&mut self, error: RuntimeError) {
        self.core_loaded = false;
        self.snapshot = RuntimeSnapshot {
            phase: RuntimePhase::Error,
            rom: None,
            error: Some(error),
        };
    }

    fn start(&mut self) -> RuntimeResult<()> {
        self.require_loaded()?;
        self.snapshot.phase = RuntimePhase::Running;
        Ok(())
    }

    fn pause(&mut self) -> RuntimeResult<()> {
        self.require_loaded()?;
        self.snapshot.phase = RuntimePhase::Paused;
        Ok(())
    }

    fn restart(&mut self) -> RuntimeResult<()> {
        self.require_loaded()?;
        self.snapshot.phase = RuntimePhase::Running;
        Ok(())
    }

    fn close(&mut self) {
        self.core_loaded = false;
        self.snapshot = RuntimeSnapshot::empty();
    }

    fn require_loaded(&self) -> RuntimeResult<()> {
        if self.core_loaded {
            Ok(())
        } else {
            Err(RuntimeError::new(
                RuntimeErrorCode::InvalidLifecycle,
                "No ROM is loaded.",
            ))
        }
    }
}

enum RuntimeCommand {
    Subscribe {
        observer: Arc<dyn RuntimeObserver>,
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    AcknowledgeFrame {
        sequence: u64,
        reply: SyncSender<RuntimeResult<()>>,
    },
    OpenRom {
        path: PathBuf,
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    Start {
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    Pause {
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    Restart {
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    Close {
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    SetKeyboardInput {
        buttons: Vec<RuntimeButton>,
        reply: SyncSender<RuntimeResult<()>>,
    },
    Snapshot {
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    #[cfg(test)]
    RunTicks {
        count: usize,
        reply: SyncSender<RuntimeResult<()>>,
    },
    #[cfg(test)]
    BufferedFrameCount {
        reply: SyncSender<RuntimeResult<usize>>,
    },
    Shutdown {
        reply: SyncSender<RuntimeResult<()>>,
    },
}

pub struct DesktopRuntime {
    sender: SyncSender<RuntimeCommand>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl DesktopRuntime {
    #[must_use]
    pub fn spawn(factory: Arc<dyn CoreFactory>) -> Self {
        let (sender, receiver) = mpsc::sync_channel(COMMAND_QUEUE_CAPACITY);
        let worker = thread::Builder::new()
            .name("gameboy-desktop-runtime".into())
            .spawn(move || run_worker(&receiver, &factory))
            .expect("desktop runtime worker starts");
        Self {
            sender,
            worker: Mutex::new(Some(worker)),
        }
    }

    pub fn snapshot(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Snapshot { reply })
    }

    pub fn subscribe(&self, observer: Arc<dyn RuntimeObserver>) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Subscribe { observer, reply })
    }

    pub fn acknowledge_frame(&self, sequence: u64) -> RuntimeResult<()> {
        self.request(|reply| RuntimeCommand::AcknowledgeFrame { sequence, reply })
    }

    pub fn open_rom(&self, path: PathBuf) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::OpenRom { path, reply })
    }

    pub fn start(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Start { reply })
    }

    pub fn pause(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Pause { reply })
    }

    pub fn restart(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Restart { reply })
    }

    pub fn close(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Close { reply })
    }

    pub fn set_keyboard_input(&self, buttons: Vec<RuntimeButton>) -> RuntimeResult<()> {
        self.request(|reply| RuntimeCommand::SetKeyboardInput { buttons, reply })
    }

    #[cfg(test)]
    fn test_only_run_ticks(&self, count: usize) -> RuntimeResult<()> {
        self.request(|reply| RuntimeCommand::RunTicks { count, reply })
    }

    #[cfg(test)]
    fn test_only_buffered_frame_count(&self) -> usize {
        self.request(|reply| RuntimeCommand::BufferedFrameCount { reply })
            .expect("runtime reports buffered frame count")
    }

    pub fn shutdown(&self) -> RuntimeResult<()> {
        let mut worker = self.worker.lock().map_err(|_| runtime_unavailable())?;
        let Some(handle) = worker.take() else {
            return Ok(());
        };
        let response = self.request(|reply| RuntimeCommand::Shutdown { reply });
        handle.join().map_err(|_| runtime_unavailable())?;
        response
    }

    fn request<T>(
        &self,
        command: impl FnOnce(SyncSender<RuntimeResult<T>>) -> RuntimeCommand,
    ) -> RuntimeResult<T> {
        let (reply, response) = mpsc::sync_channel(1);
        self.sender
            .try_send(command(reply))
            .map_err(|error| match error {
                TrySendError::Full(_) | TrySendError::Disconnected(_) => runtime_unavailable(),
            })?;
        response
            .recv_timeout(RESPONSE_TIMEOUT)
            .map_err(|_| runtime_unavailable())?
    }
}

impl Drop for DesktopRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn run_worker(receiver: &Receiver<RuntimeCommand>, factory: &Arc<dyn CoreFactory>) {
    let mut model = RuntimeModel::default();
    let mut core: Option<Box<dyn RuntimeCore>> = None;
    let mut observer: Option<Arc<dyn RuntimeObserver>> = None;
    let mut delivery = FrameDelivery::default();

    loop {
        let timeout = if model.snapshot.phase == RuntimePhase::Running {
            FRAME_INTERVAL
        } else {
            IDLE_POLL_INTERVAL
        };
        match receiver.recv_timeout(timeout) {
            Ok(command) => {
                if handle_command(
                    command,
                    factory,
                    &mut model,
                    &mut core,
                    &mut observer,
                    &mut delivery,
                ) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if model.snapshot.phase == RuntimePhase::Running {
                    run_tick(
                        &mut model,
                        core.as_deref_mut(),
                        &mut observer,
                        &mut delivery,
                    );
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn handle_command(
    command: RuntimeCommand,
    factory: &Arc<dyn CoreFactory>,
    model: &mut RuntimeModel,
    core: &mut Option<Box<dyn RuntimeCore>>,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
    delivery: &mut FrameDelivery,
) -> bool {
    match command {
        RuntimeCommand::Subscribe {
            observer: replacement,
            reply,
        } => {
            delivery.clear();
            *observer = Some(replacement);
            let result = publish_control(model, observer).map(|()| model.snapshot());
            let _ = reply.send(result);
        }
        RuntimeCommand::AcknowledgeFrame { sequence, reply } => {
            let result = acknowledge_frame(sequence, observer, delivery);
            let _ = reply.send(result);
        }
        RuntimeCommand::OpenRom { path, reply } => {
            if let Some(active) = core.as_deref_mut() {
                active.clear_input_source(KEYBOARD_INPUT_SOURCE);
            }
            *core = None;
            delivery.clear();
            model.begin_load();
            let _ = publish_control(model, observer);
            let result = load_rom(&path, factory, model, core);
            let _ = publish_control(model, observer);
            let _ = reply.send(result);
        }
        RuntimeCommand::Start { reply } => {
            let result = model.start().map(|()| model.snapshot());
            if result.is_ok() {
                let _ = publish_control(model, observer);
            }
            let _ = reply.send(result);
        }
        RuntimeCommand::Pause { reply } => {
            let result = model.pause().map(|()| model.snapshot());
            if result.is_ok() {
                let _ = publish_control(model, observer);
            }
            let _ = reply.send(result);
        }
        RuntimeCommand::Restart { reply } => {
            delivery.clear();
            let result = restart_core(model, core);
            if result.is_ok() {
                let _ = publish_control(model, observer);
            }
            let _ = reply.send(result);
        }
        RuntimeCommand::Close { reply } => {
            if let Some(active) = core.as_deref_mut() {
                active.clear_input_source(KEYBOARD_INPUT_SOURCE);
            }
            *core = None;
            delivery.clear();
            model.close();
            let _ = publish_control(model, observer);
            let _ = reply.send(Ok(model.snapshot()));
        }
        RuntimeCommand::SetKeyboardInput { buttons, reply } => {
            let result = set_keyboard_input(core.as_deref_mut(), buttons);
            let _ = reply.send(result);
        }
        RuntimeCommand::Snapshot { reply } => {
            let _ = reply.send(Ok(model.snapshot()));
        }
        #[cfg(test)]
        RuntimeCommand::RunTicks { count, reply } => {
            let result = if model.snapshot.phase == RuntimePhase::Running {
                for _ in 0..count {
                    run_tick(model, core.as_deref_mut(), observer, delivery);
                }
                Ok(())
            } else {
                Err(RuntimeError::new(
                    RuntimeErrorCode::InvalidLifecycle,
                    "The runtime must be running to execute ticks.",
                ))
            };
            let _ = reply.send(result);
        }
        #[cfg(test)]
        RuntimeCommand::BufferedFrameCount { reply } => {
            let _ = reply.send(Ok(delivery.buffered_frame_count()));
        }
        RuntimeCommand::Shutdown { reply } => {
            if let Some(active) = core.as_deref_mut() {
                active.clear_input_source(KEYBOARD_INPUT_SOURCE);
            }
            *core = None;
            delivery.clear();
            model.close();
            let _ = reply.send(Ok(()));
            return true;
        }
    }
    false
}

fn load_rom(
    path: &PathBuf,
    factory: &Arc<dyn CoreFactory>,
    model: &mut RuntimeModel,
    core: &mut Option<Box<dyn RuntimeCore>>,
) -> RuntimeResult<RuntimeSnapshot> {
    let bytes = fs::read(path).map_err(|_| {
        RuntimeError::new(
            RuntimeErrorCode::FileInaccessible,
            "The selected ROM file could not be read.",
        )
    });
    let bytes = match bytes {
        Ok(bytes) => bytes,
        Err(error) => {
            model.fail_load(error.clone());
            return Err(error);
        }
    };
    let file_name = path.file_name().map_or_else(
        || "Selected ROM".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    let mut candidate = factory.create();
    // PED-40 owns persisted battery loading and every save/flush path.
    match candidate.load_rom(&bytes, None) {
        Ok(metadata) => {
            model.finish_load(metadata, file_name);
            *core = Some(candidate);
            Ok(model.snapshot())
        }
        Err(error) => {
            let error = RuntimeError::from(error);
            model.fail_load(error.clone());
            Err(error)
        }
    }
}

fn restart_core(
    model: &mut RuntimeModel,
    core: &mut Option<Box<dyn RuntimeCore>>,
) -> RuntimeResult<RuntimeSnapshot> {
    model.require_loaded()?;
    let active = core.as_deref_mut().ok_or_else(runtime_unavailable)?;
    active.clear_input_source(KEYBOARD_INPUT_SOURCE);
    active.reset().map_err(RuntimeError::from)?;
    model.restart()?;
    Ok(model.snapshot())
}

fn set_keyboard_input(
    core: Option<&mut (dyn RuntimeCore + '_)>,
    buttons: Vec<RuntimeButton>,
) -> RuntimeResult<()> {
    let active = core.ok_or_else(|| {
        RuntimeError::new(RuntimeErrorCode::InvalidLifecycle, "No ROM is loaded.")
    })?;
    let mut state = JoypadState::default();
    for button in buttons {
        state.press(match button {
            RuntimeButton::Up => Button::Up,
            RuntimeButton::Down => Button::Down,
            RuntimeButton::Left => Button::Left,
            RuntimeButton::Right => Button::Right,
            RuntimeButton::A => Button::A,
            RuntimeButton::B => Button::B,
            RuntimeButton::Start => Button::Start,
            RuntimeButton::Select => Button::Select,
        });
    }
    active.set_input(KEYBOARD_INPUT_SOURCE, state);
    Ok(())
}

fn run_tick(
    model: &mut RuntimeModel,
    core: Option<&mut (dyn RuntimeCore + '_)>,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
    delivery: &mut FrameDelivery,
) {
    let Some(active) = core else {
        model.fail_load(runtime_unavailable());
        let _ = publish_control(model, observer);
        return;
    };
    match active.run_cycles(FRAME_CYCLE_BUDGET) {
        Ok(outcome) if outcome.frame_ready() => {
            if let Some(frame) = active.take_frame() {
                offer_frame(frame, observer, delivery);
            }
        }
        Ok(_) => {}
        Err(error) => {
            model.fail_load(RuntimeError::from(error));
            delivery.clear();
            let _ = publish_control(model, observer);
        }
    }
}

fn publish_control(
    model: &RuntimeModel,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
) -> RuntimeResult<()> {
    let Some(active) = observer.as_ref() else {
        return Ok(());
    };
    let result = active.publish_control(RuntimeEvent::Snapshot {
        snapshot: model.snapshot(),
    });
    if result.is_err() {
        *observer = None;
    }
    result
}

fn offer_frame(
    frame: Frame,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
    delivery: &mut FrameDelivery,
) {
    if observer.is_none() {
        delivery.clear();
        return;
    }
    if delivery.in_flight_sequence.is_some() {
        delivery.latest_pending = Some(frame);
        return;
    }
    publish_frame(&frame, observer, delivery);
}

fn publish_frame(
    frame: &Frame,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
    delivery: &mut FrameDelivery,
) {
    let Some(active) = observer.as_ref() else {
        delivery.clear();
        return;
    };
    let sequence = frame.sequence();
    if active.publish_frame(encode_frame_packet(frame)).is_ok() {
        delivery.in_flight_sequence = Some(sequence);
    } else {
        *observer = None;
        delivery.clear();
    }
}

fn acknowledge_frame(
    sequence: u64,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
    delivery: &mut FrameDelivery,
) -> RuntimeResult<()> {
    if delivery.in_flight_sequence != Some(sequence) {
        return Err(RuntimeError::new(
            RuntimeErrorCode::InvalidLifecycle,
            "The acknowledged frame is not awaiting presentation.",
        ));
    }
    delivery.in_flight_sequence = None;
    if let Some(frame) = delivery.latest_pending.take() {
        publish_frame(&frame, observer, delivery);
    }
    Ok(())
}

fn runtime_unavailable() -> RuntimeError {
    RuntimeError::new(
        RuntimeErrorCode::RuntimeUnavailable,
        "The desktop runtime is unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::num::NonZeroU32;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use gb_core::{
        AudioBatch, BatteryState, Button, CartridgeMetadata, CompatibilityMode, CoreError,
        EmulatorCore, Frame, InputSourceId, JoypadState, MapperKind, RunOutcome,
    };

    use super::{CoreFactory, DesktopRuntime, RuntimeCore, RuntimeModel, RuntimeObserver};
    use crate::emulator::contracts::{
        RuntimeButton, RuntimeError, RuntimeErrorCode, RuntimeEvent, RuntimePhase, RuntimeSnapshot,
    };
    use crate::emulator::mock_core::ContractMockCoreFactory;

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[derive(Default)]
    struct RecordingState {
        created: usize,
        dropped: usize,
        fail_load: Option<CoreError>,
        inputs: Vec<(InputSourceId, JoypadState)>,
        clears: Vec<InputSourceId>,
    }

    struct RecordingCore {
        state: Arc<Mutex<RecordingState>>,
        loaded: bool,
    }

    impl Drop for RecordingCore {
        fn drop(&mut self) {
            self.state.lock().expect("recording state").dropped += 1;
        }
    }

    impl EmulatorCore for RecordingCore {
        fn load_rom(
            &mut self,
            _rom: &[u8],
            _persisted: Option<&BatteryState>,
        ) -> Result<CartridgeMetadata, CoreError> {
            if let Some(error) = self
                .state
                .lock()
                .expect("recording state")
                .fail_load
                .clone()
            {
                return Err(error);
            }
            self.loaded = true;
            Ok(test_metadata())
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
            self.state
                .lock()
                .expect("recording state")
                .inputs
                .push((source, state));
        }

        fn clear_input_source(&mut self, source: InputSourceId) {
            self.state
                .lock()
                .expect("recording state")
                .clears
                .push(source);
        }

        fn take_frame(&mut self) -> Option<Frame> {
            None
        }

        fn drain_audio(&mut self) -> AudioBatch {
            AudioBatch::empty(NonZeroU32::new(48_000).expect("non-zero rate"))
        }

        fn battery_state(&self) -> Option<BatteryState> {
            None
        }
    }

    #[derive(Default)]
    struct RecordingCoreFactory {
        state: Arc<Mutex<RecordingState>>,
    }

    impl CoreFactory for RecordingCoreFactory {
        fn create(&self) -> Box<dyn RuntimeCore> {
            self.state.lock().expect("recording state").created += 1;
            Box::new(RecordingCore {
                state: Arc::clone(&self.state),
                loaded: false,
            })
        }
    }

    fn synthetic_rom_path() -> PathBuf {
        let id = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ped-37-{}-{id}.gb", std::process::id()));
        fs::write(&path, b"PED-37 synthetic ROM").expect("write synthetic ROM");
        path
    }

    #[derive(Clone, Default)]
    struct RecordingObserver {
        control: Arc<Mutex<Vec<RuntimeEvent>>>,
        frames: Arc<Mutex<Vec<Vec<u8>>>>,
        fail_frames: Arc<Mutex<bool>>,
    }

    impl RecordingObserver {
        fn frame_sequences(&self) -> Vec<u64> {
            self.frames
                .lock()
                .expect("frames")
                .iter()
                .map(|packet| u64::from_le_bytes(packet[0..8].try_into().expect("sequence header")))
                .collect()
        }
    }

    impl RuntimeObserver for RecordingObserver {
        fn publish_control(&self, event: RuntimeEvent) -> Result<(), RuntimeError> {
            self.control.lock().expect("control").push(event);
            Ok(())
        }

        fn publish_frame(&self, packet: Vec<u8>) -> Result<(), RuntimeError> {
            if *self.fail_frames.lock().expect("failure flag") {
                return Err(RuntimeError::new(
                    RuntimeErrorCode::RuntimeUnavailable,
                    "observer unavailable",
                ));
            }
            self.frames.lock().expect("frames").push(packet);
            Ok(())
        }
    }

    fn test_metadata() -> CartridgeMetadata {
        CartridgeMetadata {
            title: "Fixture".into(),
            rom_identity: "fixture".into(),
            mapper: MapperKind::RomOnly,
            compatibility: CompatibilityMode::Dmg,
            ram_size_bytes: 0,
            has_battery: false,
        }
    }

    #[test]
    fn lifecycle_transitions_are_explicit() {
        let mut model = RuntimeModel::default();
        assert_eq!(model.snapshot().phase, RuntimePhase::Empty);

        model.begin_load();
        assert_eq!(model.snapshot().phase, RuntimePhase::Loading);

        model.finish_load(test_metadata(), "fixture.gb".into());
        assert_eq!(model.snapshot().phase, RuntimePhase::Paused);

        model.start().expect("loaded ROM starts");
        assert_eq!(model.snapshot().phase, RuntimePhase::Running);

        model.pause().expect("running ROM pauses");
        assert_eq!(model.snapshot().phase, RuntimePhase::Paused);

        model.restart().expect("loaded ROM restarts");
        assert_eq!(model.snapshot().phase, RuntimePhase::Running);

        model.close();
        assert_eq!(model.snapshot(), RuntimeSnapshot::empty());
    }

    #[test]
    fn lifecycle_idempotence_and_errors_are_explicit() {
        let mut model = RuntimeModel::default();
        let error = model.start().expect_err("empty runtime cannot start");
        assert_eq!(error.code, RuntimeErrorCode::InvalidLifecycle);

        model.begin_load();
        model.finish_load(test_metadata(), "fixture.gb".into());
        model.pause().expect("already paused is idempotent");
        model.start().expect("loaded runtime starts");
        model.start().expect("already running is idempotent");
        assert_eq!(model.snapshot().phase, RuntimePhase::Running);
    }

    #[test]
    fn lifecycle_load_error_clears_rom_metadata() {
        let mut model = RuntimeModel::default();
        model.begin_load();
        model.fail_load(crate::emulator::contracts::RuntimeError::new(
            RuntimeErrorCode::InvalidRom,
            "bad fixture",
        ));

        assert_eq!(model.snapshot().phase, RuntimePhase::Error);
        assert!(model.snapshot().rom.is_none());
        assert_eq!(
            model.snapshot().error.expect("typed error").code,
            RuntimeErrorCode::InvalidRom
        );
    }

    #[test]
    fn desktop_runtime_serializes_rom_lifecycle() {
        let path = synthetic_rom_path();
        let factory = Arc::new(RecordingCoreFactory::default());
        let runtime = DesktopRuntime::spawn(factory);

        let loaded = runtime.open_rom(path.clone()).expect("synthetic ROM loads");
        assert_eq!(loaded.phase, RuntimePhase::Paused);
        assert_eq!(
            loaded.rom.expect("summary").file_name,
            path.file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned()
        );
        assert_eq!(runtime.start().expect("start").phase, RuntimePhase::Running);
        assert_eq!(runtime.pause().expect("pause").phase, RuntimePhase::Paused);
        assert_eq!(
            runtime.restart().expect("restart").phase,
            RuntimePhase::Running
        );
        assert_eq!(runtime.close().expect("close").phase, RuntimePhase::Empty);
        runtime.shutdown().expect("shutdown joins worker");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn desktop_runtime_maps_file_and_core_failures() {
        let runtime = DesktopRuntime::spawn(Arc::new(RecordingCoreFactory::default()));
        let missing = std::env::temp_dir().join("ped-37-definitely-missing.gb");
        let error = runtime.open_rom(missing).expect_err("missing file fails");
        assert_eq!(error.code, RuntimeErrorCode::FileInaccessible);
        runtime.shutdown().expect("shutdown");

        let path = synthetic_rom_path();
        let factory = Arc::new(RecordingCoreFactory::default());
        factory.state.lock().expect("state").fail_load = Some(CoreError::UnsupportedMapper(0x42));
        let runtime = DesktopRuntime::spawn(factory);
        let error = runtime
            .open_rom(path.clone())
            .expect_err("core rejects ROM");
        assert_eq!(error.code, RuntimeErrorCode::UnsupportedMapper);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn desktop_runtime_replacement_drops_the_previous_core() {
        let first = synthetic_rom_path();
        let second = synthetic_rom_path();
        let factory = Arc::new(RecordingCoreFactory::default());
        let state = Arc::clone(&factory.state);
        let runtime = DesktopRuntime::spawn(factory);
        runtime.open_rom(first.clone()).expect("first loads");
        runtime.open_rom(second.clone()).expect("second loads");

        let state = state.lock().expect("state");
        assert_eq!(state.created, 2);
        assert_eq!(state.dropped, 1);
        drop(state);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(first).expect("remove first");
        fs::remove_file(second).expect("remove second");
    }

    #[test]
    fn desktop_runtime_sends_complete_keyboard_snapshots_and_clears_on_close() {
        let path = synthetic_rom_path();
        let factory = Arc::new(RecordingCoreFactory::default());
        let state = Arc::clone(&factory.state);
        let runtime = DesktopRuntime::spawn(factory);
        runtime.open_rom(path.clone()).expect("loads");

        runtime
            .set_keyboard_input(vec![
                RuntimeButton::Left,
                RuntimeButton::A,
                RuntimeButton::Start,
            ])
            .expect("input snapshot");
        runtime
            .set_keyboard_input(vec![RuntimeButton::Left])
            .expect("release snapshot");
        runtime.close().expect("close");

        let state = state.lock().expect("state");
        let first = state.inputs[0].1;
        assert!(first.is_pressed(Button::Left));
        assert!(first.is_pressed(Button::A));
        assert!(first.is_pressed(Button::Start));
        assert!(!first.is_pressed(Button::B));
        let second = state.inputs[1].1;
        assert!(second.is_pressed(Button::Left));
        assert!(!second.is_pressed(Button::A));
        assert!(!second.is_pressed(Button::Start));
        assert!(state.clears.contains(&InputSourceId::new(1)));
        drop(state);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn desktop_runtime_shutdown_is_idempotent_and_disconnects_requests() {
        let runtime = DesktopRuntime::spawn(Arc::new(RecordingCoreFactory::default()));
        runtime.shutdown().expect("first shutdown joins");
        runtime.shutdown().expect("second shutdown is a no-op");
        assert_eq!(
            runtime.snapshot().expect_err("worker is closed").code,
            RuntimeErrorCode::RuntimeUnavailable
        );
    }

    #[test]
    fn frame_backpressure_keeps_one_in_flight_and_the_latest_pending() {
        let path = synthetic_rom_path();
        let observer = RecordingObserver::default();
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));
        runtime.open_rom(path.clone()).expect("loads");
        runtime
            .subscribe(Arc::new(observer.clone()))
            .expect("subscribe");
        runtime.start().expect("start");
        runtime
            .test_only_run_ticks(3)
            .expect("three deterministic ticks");

        assert_eq!(observer.frame_sequences(), vec![1]);
        runtime.acknowledge_frame(1).expect("ack first frame");
        assert_eq!(observer.frame_sequences(), vec![1, 3]);
        runtime.acknowledge_frame(3).expect("ack newest frame");
        assert_eq!(runtime.test_only_buffered_frame_count(), 0);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn frame_backpressure_subscription_returns_the_published_snapshot() {
        let observer = RecordingObserver::default();
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));

        let snapshot = runtime
            .subscribe(Arc::new(observer.clone()))
            .expect("subscribe");

        assert_eq!(snapshot, RuntimeSnapshot::empty());
        let events = observer.control.lock().expect("control events");
        assert!(matches!(
            events.as_slice(),
            [RuntimeEvent::Snapshot { snapshot }] if snapshot == &RuntimeSnapshot::empty()
        ));
        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn frame_backpressure_rejects_stale_ack_and_clears_on_close() {
        let path = synthetic_rom_path();
        let observer = RecordingObserver::default();
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.subscribe(Arc::new(observer)).expect("subscribe");
        runtime.start().expect("start");
        runtime.test_only_run_ticks(2).expect("ticks");
        assert_eq!(
            runtime
                .acknowledge_frame(99)
                .expect_err("wrong sequence rejected")
                .code,
            RuntimeErrorCode::InvalidLifecycle
        );
        runtime.close().expect("close");
        assert_eq!(runtime.test_only_buffered_frame_count(), 0);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn frame_backpressure_replaces_observer_without_stalling_video() {
        let path = synthetic_rom_path();
        let first = RecordingObserver::default();
        let second = RecordingObserver::default();
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.subscribe(Arc::new(first.clone())).expect("first");
        runtime.start().expect("start");
        runtime.test_only_run_ticks(1).expect("first tick");
        runtime
            .subscribe(Arc::new(second.clone()))
            .expect("replace");
        runtime.test_only_run_ticks(1).expect("second tick");

        assert_eq!(first.frame_sequences(), vec![1]);
        assert_eq!(second.frame_sequences(), vec![2]);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn frame_backpressure_detaches_a_failing_observer_without_stopping_runtime() {
        let path = synthetic_rom_path();
        let observer = RecordingObserver::default();
        *observer.fail_frames.lock().expect("failure flag") = true;
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.subscribe(Arc::new(observer)).expect("subscribe");
        runtime.start().expect("start");
        runtime
            .test_only_run_ticks(1)
            .expect("tick survives send failure");

        assert_eq!(
            runtime.snapshot().expect("runtime remains alive").phase,
            RuntimePhase::Running
        );
        assert_eq!(runtime.test_only_buffered_frame_count(), 0);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }
}
