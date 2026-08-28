use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Request, State};
use axum::http::header::{CACHE_CONTROL, HeaderName, HeaderValue, ORIGIN};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use base64::Engine as _;
use futures_util::StreamExt;
use gb_core::InputSourceId;
use tokio::sync::{Notify, oneshot, watch};
use tower_http::services::ServeDir;

use crate::{
    ClientMessage, ControllerConnectionId, ControllerEventSink, InputRateLimiter, RejectionReason,
    ServerMessage, SessionAction, SessionId, SessionMachine, SessionToken,
};

const TOKEN_BYTES: usize = 32;
const IDENTIFIER_BYTES: usize = 16;
const MAX_TEXT_BYTES: usize = 4_096;
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
const PRODUCTION_TOKEN_TTL: Duration = Duration::from_mins(10);
const PRODUCTION_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(18);
const RATE_PER_SECOND: u64 = 240;
const RATE_CAPACITY: u64 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkError {
    NoLanAddress,
    EntropyUnavailable,
    AssetsUnavailable,
    BindFailed,
    ThreadStartFailed,
    ServerUnavailable,
}

impl Display for NetworkError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::NoLanAddress => "no non-loopback LAN address is available",
            Self::EntropyUnavailable => "operating system entropy is unavailable",
            Self::AssetsUnavailable => "controller assets are unavailable",
            Self::BindFailed => "controller server could not bind its listener",
            Self::ThreadStartFailed => "controller server thread could not start",
            Self::ServerUnavailable => "controller server is unavailable",
        })
    }
}

impl Error for NetworkError {}

pub trait SessionEntropy: Send + Sync {
    /// Fills `destination` with cryptographically secure session entropy.
    ///
    /// # Errors
    ///
    /// Returns [`NetworkError::EntropyUnavailable`] if entropy cannot be generated.
    fn fill(&self, destination: &mut [u8]) -> Result<(), NetworkError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OsSessionEntropy;

impl SessionEntropy for OsSessionEntropy {
    fn fill(&self, destination: &mut [u8]) -> Result<(), NetworkError> {
        getrandom::fill(destination).map_err(|_| NetworkError::EntropyUnavailable)
    }
}

pub struct SessionServerConfig {
    pub bind_address: IpAddr,
    pub controller_assets: PathBuf,
    pub input_source: InputSourceId,
    pub token_ttl: Duration,
    pub heartbeat_timeout: Duration,
    pub entropy: Arc<dyn SessionEntropy>,
}

impl SessionServerConfig {
    /// Creates the production LAN configuration.
    ///
    /// # Errors
    ///
    /// Returns [`NetworkError::NoLanAddress`] when no suitable LAN address can be discovered.
    pub fn production(
        controller_assets: PathBuf,
        input_source: InputSourceId,
    ) -> Result<Self, NetworkError> {
        Ok(Self {
            bind_address: discover_lan_ipv4()?,
            controller_assets,
            input_source,
            token_ttl: PRODUCTION_TOKEN_TTL,
            heartbeat_timeout: PRODUCTION_HEARTBEAT_TIMEOUT,
            entropy: Arc::new(OsSessionEntropy),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingInfo {
    pub session_id: SessionId,
    pub pairing_url: String,
    pub expires_at_unix_ms: u64,
}

pub struct ControllerServer {
    shutdown_sender: Mutex<Option<oneshot::Sender<()>>>,
    server_thread: Mutex<Option<JoinHandle<()>>>,
}

impl ControllerServer {
    /// Starts one controller listener on an ephemeral port.
    ///
    /// # Errors
    ///
    /// Returns a typed [`NetworkError`] when assets, entropy, thread startup, or listener binding
    /// are unavailable.
    pub fn start(
        config: SessionServerConfig,
        sink: Arc<dyn ControllerEventSink>,
    ) -> Result<(Self, PairingInfo), NetworkError> {
        let controller_assets = fs::canonicalize(&config.controller_assets)
            .map_err(|_| NetworkError::AssetsUnavailable)?;
        let index = controller_assets.join("index.html");
        if !index.is_file() {
            return Err(NetworkError::AssetsUnavailable);
        }

        let token = generate_identifier(config.entropy.as_ref(), TOKEN_BYTES)?;
        let session_id = SessionId::new(generate_identifier(
            config.entropy.as_ref(),
            IDENTIFIER_BYTES,
        )?)
        .map_err(|_| NetworkError::EntropyUnavailable)?;
        let session_token =
            SessionToken::new(token.clone()).map_err(|_| NetworkError::EntropyUnavailable)?;
        let started_at = Instant::now();
        let expires_at = started_at
            .checked_add(config.token_ttl)
            .ok_or(NetworkError::ServerUnavailable)?;
        let expires_at_unix_ms = unix_millis_after(config.token_ttl);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);

        let thread = thread::Builder::new()
            .name("gb-controller-server".to_owned())
            .spawn(move || {
                run_server_thread(
                    config,
                    controller_assets,
                    session_token,
                    expires_at,
                    sink,
                    ready_sender,
                );
            })
            .map_err(|_| NetworkError::ThreadStartFailed)?;

        let (bound_address, shutdown_sender) = match ready_receiver.recv() {
            Ok(Ok(ready)) => ready,
            Ok(Err(error)) => {
                let _ = thread.join();
                return Err(error);
            }
            Err(_) => {
                let _ = thread.join();
                return Err(NetworkError::ThreadStartFailed);
            }
        };
        let pairing_url = format!("http://{bound_address}/?token={token}");
        Ok((
            Self {
                shutdown_sender: Mutex::new(Some(shutdown_sender)),
                server_thread: Mutex::new(Some(thread)),
            },
            PairingInfo {
                session_id,
                pairing_url,
                expires_at_unix_ms,
            },
        ))
    }

    /// Invalidates the session and synchronously stops the listener.
    ///
    /// # Errors
    ///
    /// Returns [`NetworkError::ServerUnavailable`] if synchronization state is poisoned or the
    /// server thread panics.
    pub fn shutdown(&self) -> Result<(), NetworkError> {
        let sender = self
            .shutdown_sender
            .lock()
            .map_err(|_| NetworkError::ServerUnavailable)?
            .take();
        if let Some(sender) = sender {
            let _ = sender.send(());
        }

        let thread = self
            .server_thread
            .lock()
            .map_err(|_| NetworkError::ServerUnavailable)?
            .take();
        if let Some(thread) = thread {
            thread.join().map_err(|_| NetworkError::ServerUnavailable)?;
        }
        Ok(())
    }
}

impl Drop for ControllerServer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

struct ServerState {
    token: SessionToken,
    token_valid: AtomicBool,
    expires_at: Instant,
    input_source: InputSourceId,
    heartbeat_timeout: Duration,
    advertised_origin: String,
    entropy: Arc<dyn SessionEntropy>,
    sink: Arc<dyn ControllerEventSink>,
    controller_claimed: AtomicBool,
    shutdown_sender: watch::Sender<bool>,
    socket_tasks: AtomicUsize,
    socket_tasks_finished: Notify,
}

fn run_server_thread(
    config: SessionServerConfig,
    controller_assets: PathBuf,
    token: SessionToken,
    expires_at: Instant,
    sink: Arc<dyn ControllerEventSink>,
    ready_sender: mpsc::SyncSender<Result<(SocketAddr, oneshot::Sender<()>), NetworkError>>,
) {
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    else {
        let _ = ready_sender.send(Err(NetworkError::ThreadStartFailed));
        return;
    };

    runtime.block_on(async move {
        let Ok(listener) = tokio::net::TcpListener::bind((config.bind_address, 0)).await else {
            let _ = ready_sender.send(Err(NetworkError::BindFailed));
            return;
        };
        let Ok(bound_address) = listener.local_addr() else {
            let _ = ready_sender.send(Err(NetworkError::BindFailed));
            return;
        };
        let advertised_origin = format!("http://{bound_address}");
        let (socket_shutdown_sender, _) = watch::channel(false);
        let state = Arc::new(ServerState {
            token,
            token_valid: AtomicBool::new(true),
            expires_at,
            input_source: config.input_source,
            heartbeat_timeout: config.heartbeat_timeout,
            advertised_origin,
            entropy: config.entropy,
            sink,
            controller_claimed: AtomicBool::new(false),
            shutdown_sender: socket_shutdown_sender,
            socket_tasks: AtomicUsize::new(0),
            socket_tasks_finished: Notify::new(),
        });
        let app = Router::new()
            .route("/controller", get(upgrade_controller))
            .fallback_service(
                ServeDir::new(controller_assets).append_index_html_on_directories(true),
            )
            .layer(middleware::from_fn(add_security_headers))
            .with_state(state.clone());
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        if ready_sender
            .send(Ok((bound_address, shutdown_sender)))
            .is_err()
        {
            return;
        }

        let shutdown_state = state.clone();
        let shutdown_signal = async move {
            let _ = shutdown_receiver.await;
            shutdown_state.token_valid.store(false, Ordering::Release);
            let _ = shutdown_state.shutdown_sender.send(true);
        };
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal)
        .await;

        loop {
            let notified = state.socket_tasks_finished.notified();
            if state.socket_tasks.load(Ordering::Acquire) == 0 {
                break;
            }
            notified.await;
        }
    });
}

async fn add_security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn upgrade_controller(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if let Some(origin) = headers.get(ORIGIN)
        && origin.to_str().ok() != Some(state.advertised_origin.as_str())
    {
        return StatusCode::FORBIDDEN.into_response();
    }

    state.socket_tasks.fetch_add(1, Ordering::AcqRel);
    let failed_upgrade_state = state.clone();
    upgrade
        .on_failed_upgrade(move |_error: axum::Error| {
            finish_socket_task(&failed_upgrade_state);
        })
        .on_upgrade(move |socket| async move {
            handle_socket(socket, state.clone()).await;
            finish_socket_task(&state);
        })
}

fn finish_socket_task(state: &ServerState) {
    state.socket_tasks.fetch_sub(1, Ordering::AcqRel);
    state.socket_tasks_finished.notify_waiters();
}

async fn handle_socket(mut socket: WebSocket, state: Arc<ServerState>) {
    let mut shutdown = state.shutdown_sender.subscribe();
    let Ok(connection_id) =
        generate_identifier(state.entropy.as_ref(), IDENTIFIER_BYTES).and_then(|value| {
            ControllerConnectionId::new(value).map_err(|_| NetworkError::EntropyUnavailable)
        })
    else {
        reject_and_close(&mut socket, RejectionReason::MalformedMessage).await;
        return;
    };
    let mut machine = SessionMachine::new(
        state.token.clone(),
        connection_id,
        state.input_source,
        state.expires_at,
    );

    let first_frame = tokio::select! {
        _ = shutdown.changed() => {
            let _ = socket.send(Message::Close(None)).await;
            return;
        }
        received = tokio::time::timeout(HELLO_TIMEOUT, socket.next()) => received,
    };
    let Ok(Some(Ok(frame))) = first_frame else {
        reject_and_close(&mut socket, RejectionReason::MalformedMessage).await;
        return;
    };
    let message = match parse_client_frame(frame) {
        Ok(message) => message,
        Err(reason) => {
            reject_and_close(&mut socket, reason).await;
            return;
        }
    };
    if !state.token_valid.load(Ordering::Acquire) {
        reject_and_close(&mut socket, RejectionReason::InvalidToken).await;
        return;
    }
    let action = match machine.accept_hello(message, Instant::now()) {
        Ok(action) => action,
        Err(error) => {
            reject_and_close(&mut socket, error.rejection_reason()).await;
            return;
        }
    };
    if state
        .controller_claimed
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        reject_and_close(&mut socket, RejectionReason::ControllerAlreadyConnected).await;
        return;
    }

    let SessionAction::Connected { reply, event } = action else {
        state.controller_claimed.store(false, Ordering::Release);
        reject_and_close(&mut socket, RejectionReason::MalformedMessage).await;
        return;
    };
    let connected_at = Instant::now();
    if state.sink.publish(event, connected_at).is_err() {
        close_internal_error(&mut socket).await;
        cleanup_connection(&state, &mut machine);
        return;
    }
    if !send_message(&mut socket, &reply).await {
        cleanup_connection(&state, &mut machine);
        return;
    }

    run_message_loop(&mut socket, &state, &mut shutdown, &mut machine).await;
    cleanup_connection(&state, &mut machine);
}

async fn run_message_loop(
    socket: &mut WebSocket,
    state: &ServerState,
    shutdown: &mut watch::Receiver<bool>,
    machine: &mut SessionMachine,
) {
    let mut limiter = InputRateLimiter::new(RATE_PER_SECOND, RATE_CAPACITY, Instant::now());
    loop {
        let frame = tokio::select! {
            _ = shutdown.changed() => {
                let _ = send_message(socket, &ServerMessage::ControllerDisconnected).await;
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
            received = tokio::time::timeout(state.heartbeat_timeout, socket.next()) => received,
        };
        let Ok(Some(Ok(frame))) = frame else {
            break;
        };
        if matches!(frame, Message::Close(_)) {
            break;
        }
        let message = match parse_client_frame(frame) {
            Ok(message) => message,
            Err(reason) => {
                reject_and_close(socket, reason).await;
                break;
            }
        };
        let received_at = Instant::now();
        if !limiter.allow(received_at) {
            reject_and_close(socket, RejectionReason::MalformedMessage).await;
            break;
        }
        match machine.apply(message, received_at) {
            Ok(SessionAction::Input(event)) => {
                if state.sink.publish(event, received_at).is_err() {
                    close_internal_error(socket).await;
                    break;
                }
            }
            Ok(SessionAction::Reply(reply)) => {
                if !send_message(socket, &reply).await {
                    break;
                }
            }
            Ok(SessionAction::None | SessionAction::Connected { .. }) => {
                reject_and_close(socket, RejectionReason::MalformedMessage).await;
                break;
            }
            Err(error) => {
                reject_and_close(socket, error.rejection_reason()).await;
                break;
            }
        }
    }
}

fn parse_client_frame(frame: Message) -> Result<ClientMessage, RejectionReason> {
    let Message::Text(text) = frame else {
        return Err(RejectionReason::MalformedMessage);
    };
    if text.len() > MAX_TEXT_BYTES {
        return Err(RejectionReason::MalformedMessage);
    }
    if let Ok(message) = serde_json::from_str(text.as_str()) {
        return Ok(message);
    }
    let value = serde_json::from_str::<serde_json::Value>(text.as_str())
        .map_err(|_| RejectionReason::MalformedMessage)?;
    if value.get("type").and_then(serde_json::Value::as_str) == Some("hello")
        && value
            .get("version")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|version| version != "v1")
    {
        return Err(RejectionReason::UnsupportedVersion);
    }
    Err(RejectionReason::MalformedMessage)
}

async fn send_message(socket: &mut WebSocket, message: &ServerMessage) -> bool {
    let Ok(serialized) = serde_json::to_string(message) else {
        return false;
    };
    socket.send(Message::Text(serialized.into())).await.is_ok()
}

async fn reject_and_close(socket: &mut WebSocket, reason: RejectionReason) {
    let _ = send_message(socket, &ServerMessage::Rejected { reason }).await;
    let _ = socket.send(Message::Close(None)).await;
}

async fn close_internal_error(socket: &mut WebSocket) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code: 1011,
            reason: "controller event sink failed".into(),
        })))
        .await;
}

fn cleanup_connection(state: &ServerState, machine: &mut SessionMachine) {
    if let Some(event) = machine.disconnect() {
        let _ = state.sink.publish(event, Instant::now());
    }
    state.controller_claimed.store(false, Ordering::Release);
}

fn generate_identifier(
    entropy: &dyn SessionEntropy,
    byte_count: usize,
) -> Result<String, NetworkError> {
    let mut bytes = vec![0; byte_count];
    entropy.fill(&mut bytes)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn unix_millis_after(duration: Duration) -> u64 {
    let instant = SystemTime::now()
        .checked_add(duration)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let millis = instant
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

/// Discovers the concrete non-loopback IPv4 address used for the production listener.
///
/// # Errors
///
/// Returns [`NetworkError::NoLanAddress`] when route discovery fails or resolves to an
/// unspecified or loopback address.
pub fn discover_lan_ipv4() -> Result<IpAddr, NetworkError> {
    let socket =
        UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|_| NetworkError::NoLanAddress)?;
    socket
        .connect((Ipv4Addr::new(192, 0, 2, 1), 80))
        .map_err(|_| NetworkError::NoLanAddress)?;
    let address = socket
        .local_addr()
        .map_err(|_| NetworkError::NoLanAddress)?
        .ip();
    if !address.is_ipv4() || address.is_unspecified() || address.is_loopback() {
        return Err(NetworkError::NoLanAddress);
    }
    Ok(address)
}
