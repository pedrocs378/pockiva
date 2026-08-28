use std::cell::Cell;
use std::collections::VecDeque;
#[cfg(feature = "integration-test-support")]
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use gb_core::InputSourceId;
#[cfg(feature = "integration-test-support")]
use gb_network::OsSessionEntropy;
use gb_network::{
    ControllerEvent, ControllerEventSink, ControllerEventSinkError, ControllerServer, NetworkError,
    PairingInfo, SessionServerConfig,
};

use super::contracts::{
    RemoteError, RemoteErrorCode, RemoteEvent, RemoteLatency, RemotePhase, RemoteResult,
    RemoteSnapshot,
};
use crate::emulator::runtime::DesktopRuntimeHandle;

const REMOTE_INPUT_SOURCE: InputSourceId = InputSourceId::new(2);
const MAX_LATENCY_SAMPLES: usize = 128;

/// Receives ordered snapshots.
///
/// Implementations must not synchronously invoke lifecycle or subscription methods on the same
/// [`RemoteSessionManager`]. The manager detects and rejects such reentrant calls.
pub(crate) trait RemoteObserver: Send + Sync {
    fn publish(&self, event: RemoteEvent);
}

trait RunningControllerServer: Send {
    fn shutdown(&self) -> Result<(), NetworkError>;
}

impl RunningControllerServer for ControllerServer {
    fn shutdown(&self) -> Result<(), NetworkError> {
        Self::shutdown(self)
    }
}

trait ControllerServerFactory: Send + Sync {
    fn start(
        &self,
        controller_assets: PathBuf,
        sink: Arc<dyn ControllerEventSink>,
    ) -> Result<(Box<dyn RunningControllerServer>, PairingInfo), NetworkError>;
}

trait ControllerRuntime: Send + Sync {
    fn apply_controller_event(&self, event: ControllerEvent) -> Result<(), String>;
}

impl ControllerRuntime for DesktopRuntimeHandle {
    fn apply_controller_event(&self, event: ControllerEvent) -> Result<(), String> {
        self.apply_controller_event(event)
            .map_err(|error| error.message)
    }
}

#[derive(Debug, Default)]
#[cfg_attr(test, allow(dead_code))]
struct ProductionControllerServerFactory;

impl ControllerServerFactory for ProductionControllerServerFactory {
    fn start(
        &self,
        controller_assets: PathBuf,
        sink: Arc<dyn ControllerEventSink>,
    ) -> Result<(Box<dyn RunningControllerServer>, PairingInfo), NetworkError> {
        let config = SessionServerConfig::production(controller_assets, REMOTE_INPUT_SOURCE)?;
        let (server, pairing) = ControllerServer::start(config, sink)?;
        Ok((Box::new(server), pairing))
    }
}

#[derive(Debug, Default)]
#[cfg(feature = "integration-test-support")]
struct LoopbackControllerServerFactory;

#[cfg(feature = "integration-test-support")]
impl ControllerServerFactory for LoopbackControllerServerFactory {
    fn start(
        &self,
        controller_assets: PathBuf,
        sink: Arc<dyn ControllerEventSink>,
    ) -> Result<(Box<dyn RunningControllerServer>, PairingInfo), NetworkError> {
        let config = SessionServerConfig {
            bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            controller_assets,
            input_source: REMOTE_INPUT_SOURCE,
            token_ttl: Duration::from_secs(600),
            heartbeat_timeout: Duration::from_secs(18),
            input_rate_per_second: 240,
            entropy: Arc::new(OsSessionEntropy),
        };
        let (server, pairing) = ControllerServer::start(config, sink)?;
        Ok((Box::new(server), pairing))
    }
}

struct RemoteModel {
    phase: RemotePhase,
    pairing_url: Option<String>,
    expires_at_unix_ms: Option<u64>,
    controller_id: Option<String>,
    latencies: VecDeque<Duration>,
    error: Option<RemoteError>,
}

impl Default for RemoteModel {
    fn default() -> Self {
        Self {
            phase: RemotePhase::Off,
            pairing_url: None,
            expires_at_unix_ms: None,
            controller_id: None,
            latencies: VecDeque::with_capacity(MAX_LATENCY_SAMPLES),
            error: None,
        }
    }
}

impl RemoteModel {
    fn snapshot(&self) -> RemoteSnapshot {
        RemoteSnapshot {
            phase: self.phase,
            pairing_url: self.pairing_url.clone(),
            expires_at_unix_ms: self.expires_at_unix_ms,
            controller_id: self.controller_id.clone(),
            latency: latency_snapshot(&self.latencies),
            error: self.error.clone(),
        }
    }

    fn wait_for_controller(&mut self, pairing: PairingInfo) {
        self.phase = RemotePhase::Waiting;
        self.pairing_url = Some(pairing.pairing_url);
        self.expires_at_unix_ms = Some(pairing.expires_at_unix_ms);
        self.controller_id = None;
        self.latencies.clear();
        self.error = None;
    }

    fn connect(&mut self, controller_id: String) -> bool {
        if self.phase == RemotePhase::Waiting {
            self.phase = RemotePhase::Connected;
            self.controller_id = Some(controller_id);
            return true;
        }
        false
    }

    fn disconnect(&mut self, server_active: bool) -> bool {
        if self.phase == RemotePhase::Error {
            return false;
        }
        self.controller_id = None;
        if server_active {
            self.phase = RemotePhase::Waiting;
        } else {
            self.turn_off();
        }
        true
    }

    fn record_latency(&mut self, latency: Duration) {
        if self.latencies.len() == MAX_LATENCY_SAMPLES {
            let _ = self.latencies.pop_front();
        }
        self.latencies.push_back(latency);
    }

    fn turn_off(&mut self) {
        self.phase = RemotePhase::Off;
        self.pairing_url = None;
        self.expires_at_unix_ms = None;
        self.controller_id = None;
        self.latencies.clear();
        self.error = None;
    }

    fn fail(&mut self, error: RemoteError) {
        self.phase = RemotePhase::Error;
        self.pairing_url = None;
        self.expires_at_unix_ms = None;
        self.controller_id = None;
        self.error = Some(error);
    }
}

#[derive(Default)]
struct ModelState {
    model: RemoteModel,
    observer: Option<Arc<dyn RemoteObserver>>,
    revision: u64,
}

impl ModelState {
    fn delivery(&self) -> SnapshotDelivery {
        SnapshotDelivery {
            revision: self.revision,
            snapshot: self.model.snapshot(),
            observer: self.observer.clone(),
        }
    }

    fn revise(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleOperation {
    Idle,
    Starting(u64),
    Ending(u64),
}

struct LifecycleState {
    operation: LifecycleOperation,
    epoch: u64,
    server: Option<Box<dyn RunningControllerServer>>,
    restart_required: bool,
}

impl Default for LifecycleState {
    fn default() -> Self {
        Self {
            operation: LifecycleOperation::Idle,
            epoch: 0,
            server: None,
            restart_required: false,
        }
    }
}

#[derive(Default)]
struct Lifecycle {
    state: Mutex<LifecycleState>,
    changed: Condvar,
}

struct SnapshotDelivery {
    revision: u64,
    snapshot: RemoteSnapshot,
    observer: Option<Arc<dyn RemoteObserver>>,
}

#[derive(Default)]
struct DeliveryState {
    last_revision: u64,
}

struct RemoteSessionInner {
    model: Mutex<ModelState>,
    lifecycle: Lifecycle,
    delivery: Mutex<DeliveryState>,
    runtime: Arc<dyn ControllerRuntime>,
    controller_assets: PathBuf,
    server_factory: Arc<dyn ControllerServerFactory>,
}

#[derive(Clone)]
pub(crate) struct RemoteSessionManager {
    inner: Arc<RemoteSessionInner>,
}

impl RemoteSessionManager {
    #[must_use]
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) fn new(runtime: DesktopRuntimeHandle, controller_assets: PathBuf) -> Self {
        Self::new_with_factory(
            runtime,
            controller_assets,
            Arc::new(ProductionControllerServerFactory),
        )
    }

    #[must_use]
    #[cfg(feature = "integration-test-support")]
    pub(crate) fn new_loopback(runtime: DesktopRuntimeHandle, controller_assets: PathBuf) -> Self {
        Self::new_with_factory(
            runtime,
            controller_assets,
            Arc::new(LoopbackControllerServerFactory),
        )
    }

    fn new_with_factory(
        runtime: DesktopRuntimeHandle,
        controller_assets: PathBuf,
        server_factory: Arc<dyn ControllerServerFactory>,
    ) -> Self {
        Self::new_with_dependencies(Arc::new(runtime), controller_assets, server_factory)
    }

    fn new_with_dependencies(
        runtime: Arc<dyn ControllerRuntime>,
        controller_assets: PathBuf,
        server_factory: Arc<dyn ControllerServerFactory>,
    ) -> Self {
        Self {
            inner: Arc::new(RemoteSessionInner {
                model: Mutex::new(ModelState::default()),
                lifecycle: Lifecycle::default(),
                delivery: Mutex::new(DeliveryState::default()),
                runtime,
                controller_assets,
                server_factory,
            }),
        }
    }

    pub(crate) fn start(&self) -> RemoteResult<RemoteSnapshot> {
        reject_observer_reentrancy("start")?;
        let (stale_server, epoch) = {
            let mut lifecycle = self.lock_lifecycle()?;
            match lifecycle.operation {
                LifecycleOperation::Starting(_) => {
                    return Err(invalid_lifecycle("A remote session is already starting."));
                }
                LifecycleOperation::Ending(_) => {
                    return Err(invalid_lifecycle("A remote session is ending."));
                }
                LifecycleOperation::Idle => {}
            }
            if lifecycle.server.is_some() && !lifecycle.restart_required {
                drop(lifecycle);
                return self.snapshot();
            }
            lifecycle.epoch = lifecycle.epoch.wrapping_add(1);
            let epoch = lifecycle.epoch;
            lifecycle.operation = LifecycleOperation::Starting(epoch);
            lifecycle.restart_required = false;
            (lifecycle.server.take(), epoch)
        };

        if let Some(server) = stale_server
            && let Err(error) = server.shutdown()
        {
            return self.fail_start(epoch, error);
        }

        let sink: Arc<dyn ControllerEventSink> = Arc::new(ManagerEventSink {
            manager: Arc::downgrade(&self.inner),
        });
        let result = self
            .inner
            .server_factory
            .start(self.inner.controller_assets.clone(), sink);
        match result {
            Ok((server, pairing)) => self.finish_start(epoch, server, pairing),
            Err(error) => self.fail_start(epoch, error),
        }
    }

    pub(crate) fn end(&self) -> RemoteResult<RemoteSnapshot> {
        reject_observer_reentrancy("end")?;
        let (server, epoch) = {
            let mut lifecycle = self.lock_lifecycle()?;
            while let LifecycleOperation::Starting(_) | LifecycleOperation::Ending(_) =
                lifecycle.operation
            {
                lifecycle = self
                    .inner
                    .lifecycle
                    .changed
                    .wait(lifecycle)
                    .map_err(|_| server_failed("Remote lifecycle is unavailable."))?;
            }
            lifecycle.epoch = lifecycle.epoch.wrapping_add(1);
            let epoch = lifecycle.epoch;
            lifecycle.operation = LifecycleOperation::Ending(epoch);
            (lifecycle.server.take(), epoch)
        };

        if let Some(server) = server
            && let Err(error) = server.shutdown()
        {
            let remote_error = map_network_error(error);
            self.publish_failure(remote_error.clone())?;
            self.complete_lifecycle(epoch, LifecycleOperation::Ending(epoch))?;
            return Err(remote_error);
        }

        let delivery = self.update_model(|state| state.model.turn_off())?;
        let snapshot = delivery.snapshot.clone();
        self.publish_ordered(delivery)?;
        self.complete_lifecycle(epoch, LifecycleOperation::Ending(epoch))?;
        Ok(snapshot)
    }

    pub(crate) fn shutdown(&self) -> RemoteResult<RemoteSnapshot> {
        reject_observer_reentrancy("shutdown")?;
        self.end()
    }

    pub(crate) fn snapshot(&self) -> RemoteResult<RemoteSnapshot> {
        Ok(self.lock_model()?.model.snapshot())
    }

    pub(crate) fn subscribe(
        &self,
        observer: Arc<dyn RemoteObserver>,
    ) -> RemoteResult<RemoteSnapshot> {
        reject_observer_reentrancy("subscribe")?;
        let delivery = {
            let mut state = self.lock_model()?;
            state.observer = Some(observer);
            state.revise();
            state.delivery()
        };
        let snapshot = delivery.snapshot.clone();
        self.publish_ordered(delivery)?;
        Ok(snapshot)
    }

    fn finish_start(
        &self,
        epoch: u64,
        server: Box<dyn RunningControllerServer>,
        pairing: PairingInfo,
    ) -> RemoteResult<RemoteSnapshot> {
        {
            let mut lifecycle = self.lock_lifecycle()?;
            if lifecycle.operation != LifecycleOperation::Starting(epoch) {
                drop(lifecycle);
                let _ = server.shutdown();
                return Err(invalid_lifecycle("Remote session start was superseded."));
            }
            lifecycle.server = Some(server);
            lifecycle.restart_required = false;
        };

        let delivery = self.update_model(|state| state.model.wait_for_controller(pairing))?;
        let snapshot = delivery.snapshot.clone();
        self.publish_ordered(delivery)?;
        self.complete_lifecycle(epoch, LifecycleOperation::Starting(epoch))?;
        Ok(snapshot)
    }

    fn fail_start(&self, epoch: u64, error: NetworkError) -> RemoteResult<RemoteSnapshot> {
        let remote_error = map_network_error(error);
        let delivery = self.update_model(|state| state.model.fail(remote_error.clone()))?;
        self.publish_ordered(delivery)?;
        self.complete_lifecycle(epoch, LifecycleOperation::Starting(epoch))?;
        Err(remote_error)
    }

    fn publish_failure(&self, error: RemoteError) -> RemoteResult<()> {
        let delivery = self.update_model(|state| state.model.fail(error))?;
        self.publish_ordered(delivery)?;
        Ok(())
    }

    fn update_model(&self, update: impl FnOnce(&mut ModelState)) -> RemoteResult<SnapshotDelivery> {
        let mut state = self.lock_model()?;
        update(&mut state);
        state.revise();
        Ok(state.delivery())
    }

    fn publish_ordered(&self, delivery: SnapshotDelivery) -> RemoteResult<()> {
        publish_ordered(&self.inner, delivery)
            .map_err(|()| server_failed("Remote snapshot delivery state is unavailable."))
    }

    fn complete_lifecycle(&self, epoch: u64, expected: LifecycleOperation) -> RemoteResult<()> {
        let mut lifecycle = self.lock_lifecycle()?;
        if lifecycle.epoch == epoch && lifecycle.operation == expected {
            lifecycle.operation = LifecycleOperation::Idle;
            self.inner.lifecycle.changed.notify_all();
        }
        Ok(())
    }

    fn lock_model(&self) -> RemoteResult<MutexGuard<'_, ModelState>> {
        self.inner
            .model
            .lock()
            .map_err(|_| server_failed("Remote session state is unavailable."))
    }

    fn lock_lifecycle(&self) -> RemoteResult<MutexGuard<'_, LifecycleState>> {
        self.inner
            .lifecycle
            .state
            .lock()
            .map_err(|_| server_failed("Remote lifecycle is unavailable."))
    }
}

struct ManagerEventSink {
    manager: Weak<RemoteSessionInner>,
}

impl ControllerEventSink for ManagerEventSink {
    fn publish(
        &self,
        event: ControllerEvent,
        received_at: Instant,
    ) -> Result<(), ControllerEventSinkError> {
        let manager = self.manager.upgrade().ok_or(ControllerEventSinkError)?;
        handle_controller_event(&manager, event, received_at)
    }
}

fn handle_controller_event(
    manager: &RemoteSessionInner,
    event: ControllerEvent,
    received_at: Instant,
) -> Result<(), ControllerEventSinkError> {
    if event.input_source() != REMOTE_INPUT_SOURCE {
        return Err(ControllerEventSinkError);
    }
    match event {
        ControllerEvent::Connected {
            connection_id,
            input_source,
        } => {
            let controller_id = connection_id.as_str().to_owned();
            if manager
                .model
                .lock()
                .map_err(|_| ControllerEventSinkError)?
                .model
                .phase
                == RemotePhase::Error
            {
                return Err(ControllerEventSinkError);
            }
            if let Err(message) =
                manager
                    .runtime
                    .apply_controller_event(ControllerEvent::Connected {
                        connection_id,
                        input_source,
                    })
            {
                return publish_runtime_failure(manager, message);
            }
            let delivery = update_model(manager, |state| state.model.connect(controller_id))?
                .ok_or(ControllerEventSinkError)?;
            publish_ordered(manager, delivery).map_err(|()| ControllerEventSinkError)?;
            Ok(())
        }
        event @ ControllerEvent::Message { .. } => {
            if let Err(message) = manager.runtime.apply_controller_event(event) {
                return publish_runtime_failure(manager, message);
            }
            let mut state = manager.model.lock().map_err(|_| ControllerEventSinkError)?;
            state.model.record_latency(received_at.elapsed());
            state.revise();
            Ok(())
        }
        event @ ControllerEvent::Disconnected { .. } => {
            if let Err(message) = manager.runtime.apply_controller_event(event) {
                return publish_runtime_failure(manager, message);
            }
            let server_active = manager
                .lifecycle
                .state
                .lock()
                .map_err(|_| ControllerEventSinkError)?
                .server
                .is_some();
            if let Some(delivery) =
                update_model(manager, |state| state.model.disconnect(server_active))?
            {
                publish_ordered(manager, delivery).map_err(|()| ControllerEventSinkError)?;
            }
            Ok(())
        }
    }
}

fn update_model(
    manager: &RemoteSessionInner,
    update: impl FnOnce(&mut ModelState) -> bool,
) -> Result<Option<SnapshotDelivery>, ControllerEventSinkError> {
    let mut state = manager.model.lock().map_err(|_| ControllerEventSinkError)?;
    if !update(&mut state) {
        return Ok(None);
    }
    state.revise();
    Ok(Some(state.delivery()))
}

fn publish_runtime_failure(
    manager: &RemoteSessionInner,
    message: String,
) -> Result<(), ControllerEventSinkError> {
    let error = RemoteError::new(RemoteErrorCode::RuntimeUnavailable, message);
    if let Ok(mut lifecycle) = manager.lifecycle.state.lock() {
        lifecycle.restart_required = lifecycle.server.is_some();
    }
    let delivery = update_model(manager, |state| {
        state.model.fail(error);
        true
    })?
    .ok_or(ControllerEventSinkError)?;
    publish_ordered(manager, delivery).map_err(|()| ControllerEventSinkError)?;
    Err(ControllerEventSinkError)
}

thread_local! {
    static IN_OBSERVER_DELIVERY: Cell<bool> = const { Cell::new(false) };
}

struct ObserverDeliveryGuard;

impl ObserverDeliveryGuard {
    fn enter() -> Self {
        IN_OBSERVER_DELIVERY.with(|active| active.set(true));
        Self
    }
}

impl Drop for ObserverDeliveryGuard {
    fn drop(&mut self) {
        IN_OBSERVER_DELIVERY.with(|active| active.set(false));
    }
}

fn reject_observer_reentrancy(operation: &str) -> RemoteResult<()> {
    let reentrant = IN_OBSERVER_DELIVERY.with(Cell::get);
    if reentrant {
        return Err(invalid_lifecycle(format!(
            "Remote observer callbacks cannot call {operation} synchronously."
        )));
    }
    Ok(())
}

fn publish_ordered(manager: &RemoteSessionInner, delivery: SnapshotDelivery) -> Result<(), ()> {
    let mut delivery_state = manager.delivery.lock().map_err(|_| ())?;
    if delivery.revision <= delivery_state.last_revision {
        return Ok(());
    }
    delivery_state.last_revision = delivery.revision;
    if let Some(observer) = delivery.observer {
        let _guard = ObserverDeliveryGuard::enter();
        observer.publish(RemoteEvent::Snapshot {
            snapshot: delivery.snapshot,
        });
    }
    Ok(())
}

fn latency_snapshot(samples: &VecDeque<Duration>) -> Option<RemoteLatency> {
    let last = *samples.back()?;
    let mut ordered = samples.iter().copied().collect::<Vec<_>>();
    ordered.sort_unstable();
    let rank = ordered.len().saturating_mul(95).div_ceil(100);
    let p95 = ordered[rank.saturating_sub(1)];
    Some(RemoteLatency {
        samples: u64::try_from(samples.len()).unwrap_or(u64::MAX),
        last_ms: duration_millis(last),
        p95_ms: duration_millis(p95),
    })
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn map_network_error(error: NetworkError) -> RemoteError {
    let code = match error {
        NetworkError::NoLanAddress => RemoteErrorCode::NoLanAddress,
        NetworkError::BindFailed => RemoteErrorCode::BindFailed,
        NetworkError::AssetsUnavailable => RemoteErrorCode::AssetsUnavailable,
        NetworkError::EntropyUnavailable
        | NetworkError::ThreadStartFailed
        | NetworkError::ServerUnavailable => RemoteErrorCode::ServerFailed,
    };
    RemoteError::new(code, error.to_string())
}

fn invalid_lifecycle(message: impl Into<String>) -> RemoteError {
    RemoteError::new(RemoteErrorCode::InvalidLifecycle, message)
}

fn server_failed(message: impl Into<String>) -> RemoteError {
    RemoteError::new(RemoteErrorCode::ServerFailed, message)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Barrier, mpsc};

    use gb_network::{Button, ClientMessage, ControllerConnectionId, Sequence, SessionId};

    use super::*;
    use crate::emulator::mock_core::ContractMockCoreFactory;
    use crate::emulator::runtime::DesktopRuntime;

    #[derive(Default)]
    struct FakeServerState {
        starts: AtomicUsize,
        shutdowns: AtomicUsize,
        sink: Mutex<Option<Arc<dyn ControllerEventSink>>>,
        connection: Mutex<Option<ControllerConnectionId>>,
        start_error: Mutex<Option<NetworkError>>,
    }

    impl FakeServerState {
        fn publish(
            &self,
            event: ControllerEvent,
            received_at: Instant,
        ) -> Result<(), ControllerEventSinkError> {
            match &event {
                ControllerEvent::Connected { connection_id, .. } => {
                    *self.connection.lock().expect("connection lock") = Some(connection_id.clone());
                }
                ControllerEvent::Disconnected { .. } => {
                    let _ = self.connection.lock().expect("connection lock").take();
                }
                ControllerEvent::Message { .. } => {}
            }
            self.sink
                .lock()
                .expect("sink lock")
                .as_ref()
                .expect("server sink")
                .publish(event, received_at)
        }
    }

    struct FakeServer {
        state: Arc<FakeServerState>,
    }

    impl RunningControllerServer for FakeServer {
        fn shutdown(&self) -> Result<(), NetworkError> {
            self.state.shutdowns.fetch_add(1, Ordering::SeqCst);
            let connection = self
                .state
                .connection
                .lock()
                .expect("connection lock")
                .take();
            if let Some(connection_id) = connection {
                let _ = self
                    .state
                    .sink
                    .lock()
                    .expect("sink lock")
                    .as_ref()
                    .expect("server sink")
                    .publish(
                        ControllerEvent::Disconnected {
                            connection_id,
                            input_source: REMOTE_INPUT_SOURCE,
                        },
                        Instant::now(),
                    );
            }
            Ok(())
        }
    }

    struct FakeServerFactory {
        state: Arc<FakeServerState>,
    }

    impl ControllerServerFactory for FakeServerFactory {
        fn start(
            &self,
            _controller_assets: PathBuf,
            sink: Arc<dyn ControllerEventSink>,
        ) -> Result<(Box<dyn RunningControllerServer>, PairingInfo), NetworkError> {
            self.state.starts.fetch_add(1, Ordering::SeqCst);
            if let Some(error) = self
                .state
                .start_error
                .lock()
                .expect("start error lock")
                .take()
            {
                return Err(error);
            }
            *self.state.sink.lock().expect("sink lock") = Some(sink);
            Ok((
                Box::new(FakeServer {
                    state: self.state.clone(),
                }),
                PairingInfo {
                    session_id: SessionId::new("session-1").expect("session id"),
                    pairing_url: "http://192.0.2.2:1234/?token=secret".to_owned(),
                    expires_at_unix_ms: 123_456,
                },
            ))
        }
    }

    #[derive(Default)]
    struct RecordingObserver {
        snapshots: Mutex<Vec<RemoteSnapshot>>,
    }

    impl RemoteObserver for RecordingObserver {
        fn publish(&self, event: RemoteEvent) {
            let RemoteEvent::Snapshot { snapshot } = event;
            self.snapshots.lock().expect("observer lock").push(snapshot);
        }
    }

    #[derive(Default)]
    struct RecordingRuntime {
        events: Mutex<Vec<ControllerEvent>>,
        fail: std::sync::atomic::AtomicBool,
    }

    impl ControllerRuntime for RecordingRuntime {
        fn apply_controller_event(&self, event: ControllerEvent) -> Result<(), String> {
            self.events.lock().expect("runtime events lock").push(event);
            if self.fail.load(Ordering::SeqCst) {
                Err("runtime unavailable".to_owned())
            } else {
                Ok(())
            }
        }
    }

    fn manager_with_runtime(
        runtime: Arc<dyn ControllerRuntime>,
    ) -> (RemoteSessionManager, Arc<FakeServerState>) {
        let factory_state = Arc::new(FakeServerState::default());
        let manager = RemoteSessionManager::new_with_dependencies(
            runtime,
            PathBuf::from("controller-assets"),
            Arc::new(FakeServerFactory {
                state: factory_state.clone(),
            }),
        );
        (manager, factory_state)
    }

    fn test_manager() -> (DesktopRuntime, RemoteSessionManager, Arc<FakeServerState>) {
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));
        let factory_state = Arc::new(FakeServerState::default());
        let manager = RemoteSessionManager::new_with_factory(
            runtime.handle(),
            PathBuf::from("controller-assets"),
            Arc::new(FakeServerFactory {
                state: factory_state.clone(),
            }),
        );
        (runtime, manager, factory_state)
    }

    fn connected_event() -> ControllerEvent {
        ControllerEvent::Connected {
            connection_id: ControllerConnectionId::new("controller-1").expect("connection id"),
            input_source: REMOTE_INPUT_SOURCE,
        }
    }

    fn state_sync(sequence: u64) -> ControllerEvent {
        ControllerEvent::Message {
            connection_id: ControllerConnectionId::new("controller-1").expect("connection id"),
            input_source: REMOTE_INPUT_SOURCE,
            message: ClientMessage::StateSync {
                buttons: vec![Button::Left, Button::A],
                sequence: Sequence::new(sequence).expect("safe sequence"),
            },
        }
    }

    fn disconnected_event() -> ControllerEvent {
        ControllerEvent::Disconnected {
            connection_id: ControllerConnectionId::new("controller-1").expect("connection id"),
            input_source: REMOTE_INPUT_SOURCE,
        }
    }

    #[test]
    fn manager_transitions_without_changing_rom_lifecycle_and_end_is_idempotent() {
        let (runtime, manager, server) = test_manager();
        let observer = Arc::new(RecordingObserver::default());
        assert_eq!(
            manager
                .subscribe(observer.clone())
                .expect("subscribe")
                .phase,
            RemotePhase::Off
        );
        let runtime_phase = runtime.snapshot().expect("runtime snapshot").phase;

        let waiting = manager.start().expect("start remote session");
        assert_eq!(waiting.phase, RemotePhase::Waiting);
        assert!(waiting.pairing_url.is_some());
        assert_eq!(
            manager.start().expect("active start is idempotent"),
            waiting
        );
        assert_eq!(server.starts.load(Ordering::SeqCst), 1);

        server
            .publish(connected_event(), Instant::now())
            .expect("connect event");
        assert_eq!(
            manager.snapshot().expect("connected snapshot").phase,
            RemotePhase::Connected
        );
        let received_at = Instant::now()
            .checked_sub(Duration::from_millis(10))
            .expect("test instant subtraction");
        server
            .publish(state_sync(0), received_at)
            .expect("input event");
        assert_eq!(
            manager
                .snapshot()
                .expect("latency snapshot")
                .latency
                .expect("latency")
                .samples,
            1
        );
        server
            .publish(disconnected_event(), Instant::now())
            .expect("disconnect event");
        assert_eq!(
            manager.snapshot().expect("waiting again").phase,
            RemotePhase::Waiting
        );

        server
            .publish(connected_event(), Instant::now())
            .expect("reconnect event");
        let off = manager.end().expect("end remote session");
        assert_eq!(off, RemoteSnapshot::off());
        assert_eq!(manager.end().expect("end remains idempotent"), off);
        assert_eq!(server.shutdowns.load(Ordering::SeqCst), 1);
        assert_eq!(
            runtime.snapshot().expect("runtime remains live").phase,
            runtime_phase
        );
        assert!(observer.snapshots.lock().expect("observer lock").len() >= 5);
        runtime.shutdown().expect("runtime shutdown");
    }

    #[test]
    fn latency_window_keeps_only_the_latest_128_samples() {
        let (runtime, manager, server) = test_manager();
        manager.start().expect("start remote session");
        server
            .publish(connected_event(), Instant::now())
            .expect("connect event");
        for sequence in 0..129 {
            let received_at = Instant::now()
                .checked_sub(Duration::from_millis(10))
                .expect("test instant subtraction");
            server
                .publish(state_sync(sequence), received_at)
                .expect("input event");
        }

        let latency = manager
            .snapshot()
            .expect("snapshot")
            .latency
            .expect("latency");
        assert_eq!(latency.samples, 128);
        assert!(latency.last_ms >= 10);
        assert!(latency.p95_ms >= 10);
        manager.shutdown().expect("manager shutdown");
        runtime.shutdown().expect("runtime shutdown");
    }

    #[test]
    fn start_failures_publish_typed_error_without_leaking_pairing_data() {
        let (runtime, manager, server) = test_manager();
        *server.start_error.lock().expect("start error lock") = Some(NetworkError::NoLanAddress);

        let error = manager.start().expect_err("start fails");
        assert_eq!(error.code, RemoteErrorCode::NoLanAddress);
        let snapshot = manager.snapshot().expect("error snapshot");
        assert_eq!(snapshot.phase, RemotePhase::Error);
        assert!(snapshot.pairing_url.is_none());
        assert!(snapshot.expires_at_unix_ms.is_none());
        assert_eq!(manager.end().expect("error can end"), RemoteSnapshot::off());
        runtime.shutdown().expect("runtime shutdown");
    }

    #[test]
    fn runtime_failure_closes_the_event_path_and_publishes_error() {
        let (runtime, manager, server) = test_manager();
        manager.start().expect("start remote session");
        server
            .publish(connected_event(), Instant::now())
            .expect("connect event");
        runtime.shutdown().expect("runtime shutdown");

        assert!(server.publish(state_sync(0), Instant::now()).is_err());
        let snapshot = manager.snapshot().expect("error snapshot");
        assert_eq!(snapshot.phase, RemotePhase::Error);
        assert_eq!(
            snapshot.error.expect("runtime error").code,
            RemoteErrorCode::RuntimeUnavailable
        );
        assert!(snapshot.pairing_url.is_none());
        assert!(
            server.publish(connected_event(), Instant::now()).is_err(),
            "an error-phase manager must reject a hidden controller"
        );
        let retried = manager.start().expect("error session retries");
        assert_eq!(retried.phase, RemotePhase::Waiting);
        assert_eq!(server.shutdowns.load(Ordering::SeqCst), 1);
        assert_eq!(server.starts.load(Ordering::SeqCst), 2);
        manager
            .end()
            .expect("manager shutdown after runtime failure");
    }

    struct NoopServer {
        shutdowns: Arc<AtomicUsize>,
    }

    impl RunningControllerServer for NoopServer {
        fn shutdown(&self) -> Result<(), NetworkError> {
            self.shutdowns.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct BlockingFactory {
        entered: Mutex<Option<mpsc::Sender<()>>>,
        release: Arc<Barrier>,
        shutdowns: Arc<AtomicUsize>,
    }

    impl ControllerServerFactory for BlockingFactory {
        fn start(
            &self,
            _controller_assets: PathBuf,
            _sink: Arc<dyn ControllerEventSink>,
        ) -> Result<(Box<dyn RunningControllerServer>, PairingInfo), NetworkError> {
            if let Some(entered) = self.entered.lock().expect("entered lock").take() {
                entered.send(()).expect("report start entered");
            }
            self.release.wait();
            Ok((
                Box::new(NoopServer {
                    shutdowns: self.shutdowns.clone(),
                }),
                PairingInfo {
                    session_id: SessionId::new("session-blocked").expect("session id"),
                    pairing_url: "http://192.0.2.2:1234/?token=blocked".to_owned(),
                    expires_at_unix_ms: 123_456,
                },
            ))
        }
    }

    #[test]
    fn concurrent_start_returns_invalid_lifecycle_without_waiting_on_server_start() {
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));
        let (entered_sender, entered_receiver) = mpsc::channel();
        let release = Arc::new(Barrier::new(2));
        let shutdowns = Arc::new(AtomicUsize::new(0));
        let manager = RemoteSessionManager::new_with_factory(
            runtime.handle(),
            PathBuf::from("controller-assets"),
            Arc::new(BlockingFactory {
                entered: Mutex::new(Some(entered_sender)),
                release: release.clone(),
                shutdowns,
            }),
        );
        let starting_manager = manager.clone();
        let starter = std::thread::spawn(move || starting_manager.start());
        entered_receiver.recv().expect("factory start entered");

        let error = manager.start().expect_err("second start rejected");
        assert_eq!(error.code, RemoteErrorCode::InvalidLifecycle);
        release.wait();
        assert_eq!(
            starter
                .join()
                .expect("starter joins")
                .expect("start succeeds")
                .phase,
            RemotePhase::Waiting
        );
        manager.shutdown().expect("manager shutdown");
        runtime.shutdown().expect("runtime shutdown");
    }

    #[test]
    fn connected_is_forwarded_before_status_and_error_phase_rejects_hidden_connections() {
        let runtime = Arc::new(RecordingRuntime::default());
        let (manager, server) = manager_with_runtime(runtime.clone());
        manager.start().expect("start remote session");

        server
            .publish(connected_event(), Instant::now())
            .expect("connected is forwarded");
        assert_eq!(
            manager.snapshot().expect("connected snapshot").phase,
            RemotePhase::Connected
        );
        let events = runtime.events.lock().expect("runtime events lock");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ControllerEvent::Connected { .. }));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, ControllerEvent::Disconnected { .. }))
        );
        drop(events);

        runtime.fail.store(true, Ordering::SeqCst);
        assert!(server.publish(state_sync(0), Instant::now()).is_err());
        runtime.fail.store(false, Ordering::SeqCst);
        let forwarded_before_hidden_connect =
            runtime.events.lock().expect("runtime events lock").len();
        assert!(server.publish(connected_event(), Instant::now()).is_err());
        assert_eq!(
            runtime.events.lock().expect("runtime events lock").len(),
            forwarded_before_hidden_connect,
            "error-phase connected must close before reaching the runtime"
        );
        manager.end().expect("manager end");
    }

    #[test]
    fn shutdown_waits_for_inflight_start_then_stops_the_resulting_server() {
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));
        let (entered_sender, entered_receiver) = mpsc::channel();
        let release = Arc::new(Barrier::new(2));
        let shutdowns = Arc::new(AtomicUsize::new(0));
        let manager = RemoteSessionManager::new_with_factory(
            runtime.handle(),
            PathBuf::from("controller-assets"),
            Arc::new(BlockingFactory {
                entered: Mutex::new(Some(entered_sender)),
                release: release.clone(),
                shutdowns: shutdowns.clone(),
            }),
        );
        let starting_manager = manager.clone();
        let starter = std::thread::spawn(move || starting_manager.start());
        entered_receiver.recv().expect("factory start entered");

        let (ended_sender, ended_receiver) = mpsc::channel();
        let ending_manager = manager.clone();
        let ender = std::thread::spawn(move || {
            let result = ending_manager.shutdown();
            ended_sender.send(result).expect("report shutdown");
        });
        assert!(
            ended_receiver
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "shutdown must wait for the in-flight start"
        );
        release.wait();
        starter
            .join()
            .expect("starter joins")
            .expect("start succeeds");
        assert_eq!(
            ended_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("shutdown completes")
                .expect("shutdown succeeds"),
            RemoteSnapshot::off()
        );
        ender.join().expect("ender joins");
        assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
        runtime.shutdown().expect("runtime shutdown");
    }

    struct BlockingShutdownServer {
        entered: Mutex<Option<mpsc::Sender<()>>>,
        release: Arc<Barrier>,
    }

    impl RunningControllerServer for BlockingShutdownServer {
        fn shutdown(&self) -> Result<(), NetworkError> {
            if let Some(entered) = self.entered.lock().expect("entered lock").take() {
                entered.send(()).expect("report shutdown entered");
            }
            self.release.wait();
            Ok(())
        }
    }

    struct BlockingShutdownFactory {
        starts: Arc<AtomicUsize>,
        entered: Mutex<Option<mpsc::Sender<()>>>,
        release: Arc<Barrier>,
    }

    impl ControllerServerFactory for BlockingShutdownFactory {
        fn start(
            &self,
            _controller_assets: PathBuf,
            _sink: Arc<dyn ControllerEventSink>,
        ) -> Result<(Box<dyn RunningControllerServer>, PairingInfo), NetworkError> {
            self.starts.fetch_add(1, Ordering::SeqCst);
            Ok((
                Box::new(BlockingShutdownServer {
                    entered: Mutex::new(self.entered.lock().expect("entered lock").take()),
                    release: self.release.clone(),
                }),
                PairingInfo {
                    session_id: SessionId::new("session-ending").expect("session id"),
                    pairing_url: "http://192.0.2.2:1234/?token=ending".to_owned(),
                    expires_at_unix_ms: 123_456,
                },
            ))
        }
    }

    #[test]
    fn start_during_end_is_rejected_and_cannot_leave_a_listener_alive() {
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));
        let (entered_sender, entered_receiver) = mpsc::channel();
        let release = Arc::new(Barrier::new(2));
        let starts = Arc::new(AtomicUsize::new(0));
        let manager = RemoteSessionManager::new_with_factory(
            runtime.handle(),
            PathBuf::from("controller-assets"),
            Arc::new(BlockingShutdownFactory {
                starts: starts.clone(),
                entered: Mutex::new(Some(entered_sender)),
                release: release.clone(),
            }),
        );
        manager.start().expect("initial start");
        let ending_manager = manager.clone();
        let ender = std::thread::spawn(move || ending_manager.end());
        entered_receiver.recv().expect("shutdown entered");

        let error = manager.start().expect_err("start while ending rejected");
        assert_eq!(error.code, RemoteErrorCode::InvalidLifecycle);
        assert_eq!(starts.load(Ordering::SeqCst), 1);
        release.wait();
        assert_eq!(
            ender.join().expect("ender joins").expect("end succeeds"),
            RemoteSnapshot::off()
        );
        runtime.shutdown().expect("runtime shutdown");
    }

    #[test]
    fn ordered_delivery_drops_a_stale_snapshot_after_subscribe_immediate() {
        let (_runtime, manager, _server) = test_manager();
        let observer = Arc::new(RecordingObserver::default());
        manager
            .subscribe(observer.clone())
            .expect("immediate subscription");
        let stale = SnapshotDelivery {
            revision: 2,
            snapshot: RemoteSnapshot {
                phase: RemotePhase::Waiting,
                pairing_url: Some("http://stale.invalid/?token=stale".to_owned()),
                expires_at_unix_ms: Some(1),
                controller_id: None,
                latency: None,
                error: None,
            },
            observer: Some(observer.clone()),
        };
        let newer = SnapshotDelivery {
            revision: 3,
            snapshot: RemoteSnapshot {
                phase: RemotePhase::Connected,
                pairing_url: Some("http://current.invalid/?token=current".to_owned()),
                expires_at_unix_ms: Some(2),
                controller_id: Some("controller-current".to_owned()),
                latency: None,
                error: None,
            },
            observer: Some(observer.clone()),
        };
        manager.publish_ordered(newer).expect("newer delivery");
        manager
            .publish_ordered(stale)
            .expect("stale delivery ignored");

        let phases = observer
            .snapshots
            .lock()
            .expect("observer lock")
            .iter()
            .map(|snapshot| snapshot.phase)
            .collect::<Vec<_>>();
        assert_eq!(phases, vec![RemotePhase::Off, RemotePhase::Connected]);
    }

    struct ReentrantObserver {
        manager: RemoteSessionManager,
        armed: std::sync::atomic::AtomicBool,
        results: Mutex<Vec<RemoteResult<RemoteSnapshot>>>,
    }

    impl RemoteObserver for ReentrantObserver {
        fn publish(&self, _event: RemoteEvent) {
            if self.armed.load(Ordering::SeqCst) {
                self.results
                    .lock()
                    .expect("results lock")
                    .push(self.manager.end());
            }
        }
    }

    #[test]
    fn reentrant_observer_lifecycle_call_is_rejected_without_self_join() {
        let (runtime, manager, server) = test_manager();
        let observer = Arc::new(ReentrantObserver {
            manager: manager.clone(),
            armed: std::sync::atomic::AtomicBool::new(false),
            results: Mutex::new(Vec::new()),
        });
        manager
            .subscribe(observer.clone())
            .expect("subscribe observer");
        manager.start().expect("start remote session");
        observer.armed.store(true, Ordering::SeqCst);

        server
            .publish(connected_event(), Instant::now())
            .expect("connected event does not deadlock");
        let results = observer.results.lock().expect("results lock");
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0]
                .as_ref()
                .expect_err("reentrant end rejected")
                .code,
            RemoteErrorCode::InvalidLifecycle
        );
        drop(results);
        observer.armed.store(false, Ordering::SeqCst);
        manager.end().expect("ordinary end succeeds");
        runtime.shutdown().expect("runtime shutdown");
    }
}
