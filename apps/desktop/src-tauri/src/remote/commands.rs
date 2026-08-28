use std::sync::Arc;

use tauri::State;
use tauri::ipc::Channel;

use super::contracts::{RemoteEvent, RemoteResult, RemoteSnapshot};
use super::manager::{RemoteObserver, RemoteSessionManager};

struct ChannelObserver {
    events: Channel<RemoteEvent>,
}

impl RemoteObserver for ChannelObserver {
    fn publish(&self, event: RemoteEvent) {
        let _ = self.events.send(event);
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn remote_snapshot(
    state: State<'_, RemoteSessionManager>,
) -> RemoteResult<RemoteSnapshot> {
    remote_snapshot_impl(&state)
}

pub(crate) fn remote_snapshot_impl(manager: &RemoteSessionManager) -> RemoteResult<RemoteSnapshot> {
    manager.snapshot()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn subscribe_remote(
    events: Channel<RemoteEvent>,
    state: State<'_, RemoteSessionManager>,
) -> RemoteResult<RemoteSnapshot> {
    subscribe_remote_impl(events, &state)
}

pub(crate) fn subscribe_remote_impl(
    events: Channel<RemoteEvent>,
    manager: &RemoteSessionManager,
) -> RemoteResult<RemoteSnapshot> {
    manager.subscribe(Arc::new(ChannelObserver { events }))
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn start_remote_session(
    state: State<'_, RemoteSessionManager>,
) -> RemoteResult<RemoteSnapshot> {
    start_remote_session_impl(&state)
}

pub(crate) fn start_remote_session_impl(
    manager: &RemoteSessionManager,
) -> RemoteResult<RemoteSnapshot> {
    manager.start()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn end_remote_session(
    state: State<'_, RemoteSessionManager>,
) -> RemoteResult<RemoteSnapshot> {
    end_remote_session_impl(&state)
}

pub(crate) fn end_remote_session_impl(
    manager: &RemoteSessionManager,
) -> RemoteResult<RemoteSnapshot> {
    manager.end()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use tauri::ipc::Channel;

    use super::{end_remote_session_impl, remote_snapshot_impl, subscribe_remote_impl};
    use crate::emulator::mock_core::ContractMockCoreFactory;
    use crate::emulator::runtime::DesktopRuntime;
    use crate::remote::contracts::RemotePhase;
    use crate::remote::manager::RemoteSessionManager;

    #[test]
    fn command_adapters_snapshot_subscribe_and_end_without_business_logic() {
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));
        let manager =
            RemoteSessionManager::new(runtime.handle(), PathBuf::from("unused-controller-assets"));
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&events);
        let channel = Channel::new(move |body| {
            captured.lock().expect("event capture").push(body);
            Ok(())
        });

        assert_eq!(
            remote_snapshot_impl(&manager).expect("snapshot").phase,
            RemotePhase::Off
        );
        assert_eq!(
            subscribe_remote_impl(channel, &manager)
                .expect("subscribe")
                .phase,
            RemotePhase::Off
        );
        assert_eq!(events.lock().expect("events").len(), 1);
        assert_eq!(
            end_remote_session_impl(&manager)
                .expect("idempotent end")
                .phase,
            RemotePhase::Off
        );

        runtime.shutdown().expect("runtime shutdown");
    }
}
