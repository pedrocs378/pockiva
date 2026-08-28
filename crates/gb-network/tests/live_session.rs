use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use gb_core::InputSourceId;
use gb_network::{
    ClientMessage, ControllerEvent, ControllerEventSink, ControllerEventSinkError,
    ControllerServer, NetworkError, SessionEntropy, SessionServerConfig,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

#[derive(Debug)]
struct TestAssets {
    root: PathBuf,
}

impl TestAssets {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "gb-network-live-session-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create controller asset root");
        fs::write(
            root.join("index.html"),
            "<!doctype html><title>Controller</title>",
        )
        .expect("write index");
        Self { root }
    }
}

impl Drop for TestAssets {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug)]
struct FixedEntropy(u8);

impl SessionEntropy for FixedEntropy {
    fn fill(&self, destination: &mut [u8]) -> Result<(), NetworkError> {
        destination.fill(self.0);
        Ok(())
    }
}

#[derive(Debug, Default)]
struct RecordingSink {
    events: Mutex<Vec<ControllerEvent>>,
}

#[derive(Debug)]
struct DelayedRecordingSink {
    events: Mutex<Vec<ControllerEvent>>,
    message_delay: Duration,
}

#[derive(Debug)]
struct SlowDisconnectSink {
    disconnect_delay: Duration,
}

impl ControllerEventSink for SlowDisconnectSink {
    fn publish(
        &self,
        event: ControllerEvent,
        _received_at: std::time::Instant,
    ) -> Result<(), ControllerEventSinkError> {
        if matches!(event, ControllerEvent::Disconnected { .. }) {
            std::thread::sleep(self.disconnect_delay);
        }
        Ok(())
    }
}

impl DelayedRecordingSink {
    fn new(message_delay: Duration) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            message_delay,
        }
    }

    fn events(&self) -> Vec<ControllerEvent> {
        self.events.lock().expect("events lock").clone()
    }
}

impl ControllerEventSink for DelayedRecordingSink {
    fn publish(
        &self,
        event: ControllerEvent,
        _received_at: std::time::Instant,
    ) -> Result<(), ControllerEventSinkError> {
        self.events.lock().expect("events lock").push(event.clone());
        if matches!(event, ControllerEvent::Message { .. }) {
            std::thread::sleep(self.message_delay);
        }
        Ok(())
    }
}

impl RecordingSink {
    fn events(&self) -> Vec<ControllerEvent> {
        self.events.lock().expect("events lock").clone()
    }
}

impl ControllerEventSink for RecordingSink {
    fn publish(
        &self,
        event: ControllerEvent,
        _received_at: std::time::Instant,
    ) -> Result<(), ControllerEventSinkError> {
        self.events.lock().expect("events lock").push(event);
        Ok(())
    }
}

fn config(assets: &Path, entropy: u8) -> SessionServerConfig {
    SessionServerConfig {
        bind_address: "127.0.0.1".parse().expect("loopback"),
        controller_assets: assets.to_owned(),
        input_source: InputSourceId::new(2),
        token_ttl: Duration::from_secs(600),
        heartbeat_timeout: Duration::from_secs(18),
        input_rate_per_second: 240,
        entropy: Arc::new(FixedEntropy(entropy)),
    }
}

fn token_from_pairing_url(pairing_url: &str) -> &str {
    pairing_url
        .split_once("?token=")
        .map(|(_, token)| token)
        .expect("pairing token")
}

fn socket_address(pairing_url: &str) -> &str {
    pairing_url
        .strip_prefix("http://")
        .and_then(|url| url.split('/').next())
        .expect("pairing socket address")
}

async fn http_get(pairing_url: &str, path: &str) -> io::Result<String> {
    let address = socket_address(pairing_url);
    let mut stream = tokio::net::TcpStream::connect(address).await?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await?;
    let mut response = String::new();
    stream.read_to_string(&mut response).await?;
    Ok(response)
}

async fn next_json(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> serde_json::Value {
    let message = socket
        .next()
        .await
        .expect("server response")
        .expect("valid websocket response");
    let Message::Text(text) = message else {
        panic!("expected text response, got {message:?}");
    };
    serde_json::from_str(&text).expect("JSON response")
}

async fn send_json(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    value: serde_json::Value,
) {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .expect("send JSON");
}

async fn connect_controller(
    pairing_url: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let ws_url = format!("ws://{}/controller", socket_address(pairing_url));
    tokio_tungstenite::connect_async(ws_url)
        .await
        .expect("connect websocket")
        .0
}

async fn authenticate(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    token: &str,
) {
    send_json(
        socket,
        serde_json::json!({"type":"hello","version":"v1","token":token}),
    )
    .await;
    assert_eq!(next_json(socket).await["type"], "welcome");
}

async fn wait_for_event_count(sink: &RecordingSink, expected: usize) -> Vec<ControllerEvent> {
    for _ in 0..100 {
        let events = sink.events();
        if events.len() >= expected {
            return events;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {expected} events");
}

async fn wait_for_delayed_event_count(
    sink: &DelayedRecordingSink,
    expected: usize,
) -> Vec<ControllerEvent> {
    for _ in 0..300 {
        let events = sink.events();
        if events.len() >= expected {
            return events;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {expected} delayed events");
}

#[tokio::test]
async fn serves_assets_and_drives_one_authenticated_controller_over_a_real_socket() {
    let assets = TestAssets::new();
    let sink = Arc::new(RecordingSink::default());
    let (server, pairing) = ControllerServer::start(config(&assets.root, 0xab), sink.clone())
        .expect("controller server starts");
    let expected_token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0xab; 32]);
    assert_eq!(token_from_pairing_url(&pairing.pairing_url), expected_token);

    let response = http_get(&pairing.pairing_url, "/")
        .await
        .expect("GET index");
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("<title>Controller</title>"));
    assert!(
        response
            .to_ascii_lowercase()
            .contains("cache-control: no-store")
    );

    let mut socket = connect_controller(&pairing.pairing_url).await;
    authenticate(&mut socket, &expected_token).await;

    send_json(
        &mut socket,
        serde_json::json!({"type":"state-sync","buttons":["left","a"],"sequence":7}),
    )
    .await;
    send_json(
        &mut socket,
        serde_json::json!({"type":"button-up","button":"a","sequence":8}),
    )
    .await;
    send_json(&mut socket, serde_json::json!({"type":"ping","sequence":9})).await;
    assert_eq!(
        next_json(&mut socket).await,
        serde_json::json!({"type":"pong","sequence":9})
    );

    socket.close(None).await.expect("close websocket");
    let events = wait_for_event_count(&sink, 4).await;
    assert!(matches!(events[0], ControllerEvent::Connected { .. }));
    assert!(matches!(
        events[1],
        ControllerEvent::Message {
            message: ClientMessage::StateSync { .. },
            ..
        }
    ));
    assert!(matches!(
        events[2],
        ControllerEvent::Message {
            message: ClientMessage::ButtonUp { .. },
            ..
        }
    ));
    assert!(matches!(events[3], ControllerEvent::Disconnected { .. }));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ControllerEvent::Disconnected { .. }))
            .count(),
        1
    );
    server.shutdown().expect("server shutdown");
}

#[test]
fn missing_asset_root_is_rejected_before_startup() {
    let sink = Arc::new(RecordingSink::default());
    let result = ControllerServer::start(
        config(Path::new("/definitely/not/a/controller/root"), 0xcd),
        sink,
    );
    assert!(matches!(result, Err(NetworkError::AssetsUnavailable)));
}

#[test]
fn symlinks_in_the_static_asset_tree_are_rejected_before_startup() {
    let assets = TestAssets::new();
    let outside = assets.root.with_extension("outside.txt");
    fs::write(&outside, "outside controller root").expect("write outside file");
    let link = assets.root.join("escaped.txt");
    if let Err(error) = create_file_symlink(&outside, &link) {
        let _ = fs::remove_file(&outside);
        if cfg!(windows) && error.kind() == io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("create asset symlink: {error}");
    }

    let result = ControllerServer::start(
        config(&assets.root, 0xce),
        Arc::new(RecordingSink::default()),
    );
    let _ = fs::remove_file(&outside);
    assert!(matches!(result, Err(NetworkError::AssetsUnavailable)));
}

#[cfg(unix)]
fn create_file_symlink(original: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(original, link)
}

#[cfg(windows)]
fn create_file_symlink(original: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(original, link)
}

#[tokio::test]
async fn rejects_invalid_and_expired_tokens() {
    let assets = TestAssets::new();
    let sink = Arc::new(RecordingSink::default());
    let (server, pairing) = ControllerServer::start(config(&assets.root, 0x11), sink.clone())
        .expect("controller server starts");
    let mut socket = connect_controller(&pairing.pairing_url).await;
    send_json(
        &mut socket,
        serde_json::json!({"type":"hello","version":"v1","token":"wrong"}),
    )
    .await;
    assert_eq!(
        next_json(&mut socket).await,
        serde_json::json!({"type":"rejected","reason":"invalid-token"})
    );
    assert!(sink.events().is_empty());

    let mut unsupported = connect_controller(&pairing.pairing_url).await;
    send_json(
        &mut unsupported,
        serde_json::json!({
            "type":"hello",
            "version":"v2",
            "token":token_from_pairing_url(&pairing.pairing_url)
        }),
    )
    .await;
    assert_eq!(
        next_json(&mut unsupported).await,
        serde_json::json!({"type":"rejected","reason":"unsupported-version"})
    );
    assert!(sink.events().is_empty());
    server.shutdown().expect("server shutdown");

    let mut expired_config = config(&assets.root, 0x12);
    expired_config.token_ttl = Duration::ZERO;
    let (expired_server, expired_pairing) =
        ControllerServer::start(expired_config, sink.clone()).expect("expired server starts");
    let mut socket = connect_controller(&expired_pairing.pairing_url).await;
    send_json(
        &mut socket,
        serde_json::json!({
            "type":"hello",
            "version":"v1",
            "token":token_from_pairing_url(&expired_pairing.pairing_url)
        }),
    )
    .await;
    assert_eq!(
        next_json(&mut socket).await,
        serde_json::json!({"type":"rejected","reason":"invalid-token"})
    );
    assert!(sink.events().is_empty());
    expired_server.shutdown().expect("expired server shutdown");
}

#[tokio::test]
async fn rejects_a_second_controller_without_disturbing_the_active_one() {
    let assets = TestAssets::new();
    let sink = Arc::new(RecordingSink::default());
    let (server, pairing) = ControllerServer::start(config(&assets.root, 0x21), sink.clone())
        .expect("controller server starts");
    let token = token_from_pairing_url(&pairing.pairing_url);
    let mut active = connect_controller(&pairing.pairing_url).await;
    authenticate(&mut active, token).await;

    let mut second = connect_controller(&pairing.pairing_url).await;
    send_json(
        &mut second,
        serde_json::json!({"type":"hello","version":"v1","token":token}),
    )
    .await;
    assert_eq!(
        next_json(&mut second).await,
        serde_json::json!({"type":"rejected","reason":"controller-already-connected"})
    );

    send_json(&mut active, serde_json::json!({"type":"ping","sequence":0})).await;
    assert_eq!(
        next_json(&mut active).await,
        serde_json::json!({"type":"pong","sequence":0})
    );
    active.close(None).await.expect("close active controller");
    let events = wait_for_event_count(&sink, 2).await;
    assert!(matches!(events[0], ControllerEvent::Connected { .. }));
    assert!(matches!(events[1], ControllerEvent::Disconnected { .. }));
    server.shutdown().expect("server shutdown");
}

#[tokio::test]
async fn rejects_wrong_origin_without_upgrading() {
    let assets = TestAssets::new();
    let sink = Arc::new(RecordingSink::default());
    let (server, pairing) = ControllerServer::start(config(&assets.root, 0x31), sink)
        .expect("controller server starts");
    let ws_url = format!("ws://{}/controller", socket_address(&pairing.pairing_url));
    let mut request = ws_url.into_client_request().expect("websocket request");
    request
        .headers_mut()
        .insert("origin", "http://attacker.invalid".parse().expect("origin"));
    let error = tokio_tungstenite::connect_async(request)
        .await
        .expect_err("wrong origin rejected");
    let status = match error {
        tokio_tungstenite::tungstenite::Error::Http(response) => response.status(),
        other => panic!("expected HTTP rejection, got {other:?}"),
    };
    assert_eq!(status, 403);
    server.shutdown().expect("server shutdown");
}

#[tokio::test]
async fn rejects_oversized_binary_malformed_and_out_of_order_messages() {
    let assets = TestAssets::new();
    let sink = Arc::new(RecordingSink::default());
    let (server, pairing) = ControllerServer::start(config(&assets.root, 0x41), sink.clone())
        .expect("controller server starts");
    let token = token_from_pairing_url(&pairing.pairing_url).to_owned();

    let event_count_before_connection = sink.events().len();
    let mut oversized = connect_controller(&pairing.pairing_url).await;
    authenticate(&mut oversized, &token).await;
    oversized
        .send(Message::Text("x".repeat(4_097).into()))
        .await
        .expect("send oversized frame");
    assert_eq!(
        next_json(&mut oversized).await,
        serde_json::json!({"type":"rejected","reason":"malformed-message"})
    );
    let terminal = tokio::time::timeout(Duration::from_secs(1), oversized.next())
        .await
        .expect("server closes after rejecting oversized frame");
    assert!(
        matches!(terminal, None | Some(Err(_) | Ok(Message::Close(_)))),
        "oversized frame must close after the protocol rejection: {terminal:?}"
    );
    let _ = wait_for_event_count(&sink, event_count_before_connection + 2).await;

    let event_count_before_connection = sink.events().len();
    let mut beyond_transport_bound = connect_controller(&pairing.pairing_url).await;
    authenticate(&mut beyond_transport_bound, &token).await;
    beyond_transport_bound
        .send(Message::Text("x".repeat(4_098).into()))
        .await
        .expect("send frame beyond transport bound");
    let terminal = tokio::time::timeout(Duration::from_secs(1), beyond_transport_bound.next())
        .await
        .expect("transport closes a frame beyond its bound promptly");
    assert!(
        matches!(terminal, None | Some(Err(_) | Ok(Message::Close(_)))),
        "frame beyond transport bound must close before application parsing: {terminal:?}"
    );
    let _ = wait_for_event_count(&sink, event_count_before_connection + 2).await;

    for invalid_frame in [
        Message::Binary(vec![0, 1, 2].into()),
        Message::Text("not-json".into()),
    ] {
        let event_count_before_connection = sink.events().len();
        let mut socket = connect_controller(&pairing.pairing_url).await;
        authenticate(&mut socket, &token).await;
        socket
            .send(invalid_frame)
            .await
            .expect("send invalid frame");
        assert_eq!(
            next_json(&mut socket).await,
            serde_json::json!({"type":"rejected","reason":"malformed-message"})
        );
        let _ = wait_for_event_count(&sink, event_count_before_connection + 2).await;
    }

    let mut socket = connect_controller(&pairing.pairing_url).await;
    authenticate(&mut socket, &token).await;
    send_json(
        &mut socket,
        serde_json::json!({"type":"state-sync","buttons":[],"sequence":10}),
    )
    .await;
    send_json(
        &mut socket,
        serde_json::json!({"type":"ping","sequence":12}),
    )
    .await;
    assert_eq!(
        next_json(&mut socket).await,
        serde_json::json!({"type":"rejected","reason":"malformed-message"})
    );
    server.shutdown().expect("server shutdown");
}

#[tokio::test]
async fn rejects_a_burst_beyond_the_token_bucket_capacity() {
    let assets = TestAssets::new();
    let sink = Arc::new(RecordingSink::default());
    let mut session_config = config(&assets.root, 0x51);
    session_config.input_rate_per_second = 0;
    let (server, pairing) =
        ControllerServer::start(session_config, sink).expect("controller server starts");
    let token = token_from_pairing_url(&pairing.pairing_url).to_owned();
    let mut socket = connect_controller(&pairing.pairing_url).await;
    authenticate(&mut socket, &token).await;

    for sequence in 0..65 {
        send_json(
            &mut socket,
            serde_json::json!({"type":"state-sync","buttons":[],"sequence":sequence}),
        )
        .await;
    }
    assert_eq!(
        next_json(&mut socket).await,
        serde_json::json!({"type":"rejected","reason":"malformed-message"})
    );
    server.shutdown().expect("server shutdown");
}

#[tokio::test]
async fn heartbeat_timeout_cleans_up_and_allows_reconnection() {
    let assets = TestAssets::new();
    let sink = Arc::new(RecordingSink::default());
    let mut session_config = config(&assets.root, 0x61);
    session_config.heartbeat_timeout = Duration::from_millis(30);
    let (server, pairing) =
        ControllerServer::start(session_config, sink.clone()).expect("controller server starts");
    let token = token_from_pairing_url(&pairing.pairing_url).to_owned();
    let mut first = connect_controller(&pairing.pairing_url).await;
    authenticate(&mut first, &token).await;
    let events = wait_for_event_count(&sink, 2).await;
    assert!(matches!(events[1], ControllerEvent::Disconnected { .. }));

    let mut second = connect_controller(&pairing.pairing_url).await;
    authenticate(&mut second, &token).await;
    second.close(None).await.expect("close reconnected socket");
    let events = wait_for_event_count(&sink, 4).await;
    assert!(matches!(events[2], ControllerEvent::Connected { .. }));
    assert!(matches!(events[3], ControllerEvent::Disconnected { .. }));
    server.shutdown().expect("server shutdown");
}

#[tokio::test]
async fn heartbeat_deadline_is_measured_from_receive_time_not_after_sink_completion() {
    let assets = TestAssets::new();
    let sink = Arc::new(DelayedRecordingSink::new(Duration::from_millis(600)));
    let mut session_config = config(&assets.root, 0x62);
    session_config.heartbeat_timeout = Duration::from_millis(500);
    let (server, pairing) =
        ControllerServer::start(session_config, sink.clone()).expect("controller server starts");
    let token = token_from_pairing_url(&pairing.pairing_url).to_owned();
    let mut socket = connect_controller(&pairing.pairing_url).await;
    authenticate(&mut socket, &token).await;

    let sent_at = std::time::Instant::now();
    send_json(
        &mut socket,
        serde_json::json!({"type":"state-sync","buttons":[],"sequence":0}),
    )
    .await;
    let events = wait_for_delayed_event_count(&sink, 3).await;
    assert!(matches!(events[2], ControllerEvent::Disconnected { .. }));
    assert!(
        sent_at.elapsed() < Duration::from_millis(900),
        "deadline restarted after the sink completed: {:?}",
        sent_at.elapsed()
    );
    server.shutdown().expect("server shutdown");
}

#[tokio::test]
async fn explicit_shutdown_notifies_controller_and_cleans_up_once() {
    let assets = TestAssets::new();
    let sink = Arc::new(RecordingSink::default());
    let (server, pairing) = ControllerServer::start(config(&assets.root, 0x71), sink.clone())
        .expect("controller server starts");
    let token = token_from_pairing_url(&pairing.pairing_url).to_owned();
    let mut socket = connect_controller(&pairing.pairing_url).await;
    authenticate(&mut socket, &token).await;

    server.shutdown().expect("server shutdown");
    assert_eq!(
        next_json(&mut socket).await,
        serde_json::json!({"type":"controller-disconnected"})
    );
    server.shutdown().expect("repeated shutdown is idempotent");
    let events = sink.events();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ControllerEvent::Disconnected { .. }))
            .count(),
        1
    );
}

#[tokio::test]
async fn concurrent_shutdown_callers_both_wait_for_server_cleanup() {
    let assets = TestAssets::new();
    let sink = Arc::new(SlowDisconnectSink {
        disconnect_delay: Duration::from_millis(400),
    });
    let (server, pairing) = ControllerServer::start(config(&assets.root, 0x72), sink)
        .expect("controller server starts");
    let server = Arc::new(server);
    let token = token_from_pairing_url(&pairing.pairing_url).to_owned();
    let mut socket = connect_controller(&pairing.pairing_url).await;
    authenticate(&mut socket, &token).await;

    let barrier = Arc::new(std::sync::Barrier::new(3));
    let (finished_sender, finished_receiver) = std::sync::mpsc::channel();
    let callers = (0..2)
        .map(|_| {
            let server = server.clone();
            let barrier = barrier.clone();
            let finished_sender = finished_sender.clone();
            std::thread::spawn(move || {
                barrier.wait();
                server.shutdown().expect("concurrent shutdown");
                finished_sender
                    .send(())
                    .expect("report shutdown completion");
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    assert!(
        finished_receiver
            .recv_timeout(Duration::from_millis(200))
            .is_err(),
        "a concurrent shutdown returned before controller cleanup completed"
    );
    for caller in callers {
        caller.join().expect("shutdown caller joins");
    }
}

#[tokio::test]
async fn missing_static_paths_and_directory_traversal_return_not_found() {
    let assets = TestAssets::new();
    let sink = Arc::new(RecordingSink::default());
    let (server, pairing) = ControllerServer::start(config(&assets.root, 0x81), sink)
        .expect("controller server starts");

    for path in ["/not-present", "/..%2f..%2fCargo.toml"] {
        let response = http_get(&pairing.pairing_url, path)
            .await
            .expect("GET missing path");
        assert!(response.starts_with("HTTP/1.1 404"), "{response}");
        assert!(!response.contains("[workspace]"));
    }
    server.shutdown().expect("server shutdown");
}
