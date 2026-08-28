use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;
use tauri::ipc::{Channel, InvokeResponseBody, Response};

use super::contracts::{
    RuntimeButton, RuntimeError, RuntimeErrorCode, RuntimeEvent, RuntimeResult, RuntimeSnapshot,
};
use super::runtime::{DesktopRuntime, RuntimeObserver};

struct ChannelObserver {
    events: Channel<RuntimeEvent>,
    frames: Channel<Response>,
}

impl ChannelObserver {
    fn new(events: Channel<RuntimeEvent>, frames: Channel<Response>) -> Self {
        Self { events, frames }
    }
}

impl RuntimeObserver for ChannelObserver {
    fn publish_control(&self, event: RuntimeEvent) -> RuntimeResult<()> {
        self.events.send(event).map_err(channel_unavailable)
    }

    fn publish_frame(&self, packet: Vec<u8>) -> RuntimeResult<()> {
        self.frames
            .send(Response::new(InvokeResponseBody::Raw(packet)))
            .map_err(channel_unavailable)
    }
}

fn channel_unavailable(_error: tauri::Error) -> RuntimeError {
    RuntimeError::new(
        RuntimeErrorCode::RuntimeUnavailable,
        "The desktop runtime channel is unavailable.",
    )
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn runtime_snapshot(state: State<'_, DesktopRuntime>) -> RuntimeResult<RuntimeSnapshot> {
    runtime_snapshot_impl(&state)
}

pub(crate) fn runtime_snapshot_impl(runtime: &DesktopRuntime) -> RuntimeResult<RuntimeSnapshot> {
    runtime.snapshot()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn subscribe_runtime(
    events: Channel<RuntimeEvent>,
    frames: Channel<Response>,
    state: State<'_, DesktopRuntime>,
) -> RuntimeResult<RuntimeSnapshot> {
    subscribe_runtime_impl(events, frames, &state)
}

pub(crate) fn subscribe_runtime_impl(
    events: Channel<RuntimeEvent>,
    frames: Channel<Response>,
    runtime: &DesktopRuntime,
) -> RuntimeResult<RuntimeSnapshot> {
    runtime.subscribe(Arc::new(ChannelObserver::new(events, frames)))
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn open_rom(path: PathBuf, state: State<'_, DesktopRuntime>) -> RuntimeResult<RuntimeSnapshot> {
    open_rom_impl(path, &state)
}

pub(crate) fn open_rom_impl(
    path: PathBuf,
    runtime: &DesktopRuntime,
) -> RuntimeResult<RuntimeSnapshot> {
    runtime.open_rom(path)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn start_rom(state: State<'_, DesktopRuntime>) -> RuntimeResult<RuntimeSnapshot> {
    start_rom_impl(&state)
}

pub(crate) fn start_rom_impl(runtime: &DesktopRuntime) -> RuntimeResult<RuntimeSnapshot> {
    runtime.start()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn pause_rom(state: State<'_, DesktopRuntime>) -> RuntimeResult<RuntimeSnapshot> {
    pause_rom_impl(&state)
}

pub(crate) fn pause_rom_impl(runtime: &DesktopRuntime) -> RuntimeResult<RuntimeSnapshot> {
    runtime.pause()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn restart_rom(state: State<'_, DesktopRuntime>) -> RuntimeResult<RuntimeSnapshot> {
    restart_rom_impl(&state)
}

pub(crate) fn restart_rom_impl(runtime: &DesktopRuntime) -> RuntimeResult<RuntimeSnapshot> {
    runtime.restart()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn close_rom(state: State<'_, DesktopRuntime>) -> RuntimeResult<RuntimeSnapshot> {
    close_rom_impl(&state)
}

pub(crate) fn close_rom_impl(runtime: &DesktopRuntime) -> RuntimeResult<RuntimeSnapshot> {
    runtime.close()
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn set_keyboard_input(
    buttons: Vec<RuntimeButton>,
    state: State<'_, DesktopRuntime>,
) -> RuntimeResult<()> {
    set_keyboard_input_impl(buttons, &state)
}

pub(crate) fn set_keyboard_input_impl(
    buttons: Vec<RuntimeButton>,
    runtime: &DesktopRuntime,
) -> RuntimeResult<()> {
    runtime.set_keyboard_input(buttons)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn acknowledge_frame(sequence: u64, state: State<'_, DesktopRuntime>) -> RuntimeResult<()> {
    acknowledge_frame_impl(sequence, &state)
}

pub(crate) fn acknowledge_frame_impl(sequence: u64, runtime: &DesktopRuntime) -> RuntimeResult<()> {
    runtime.acknowledge_frame(sequence)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Arc, Mutex};

    use tauri::ipc::{Channel, InvokeResponseBody, Response};

    use super::{
        ChannelObserver, RuntimeObserver, close_rom_impl, open_rom_impl, pause_rom_impl,
        restart_rom_impl, runtime_snapshot_impl, set_keyboard_input_impl, start_rom_impl,
        subscribe_runtime_impl,
    };
    use crate::emulator::contracts::{RuntimeButton, RuntimeEvent, RuntimePhase};
    use crate::emulator::mock_core::ContractMockCoreFactory;
    use crate::emulator::runtime::DesktopRuntime;

    #[test]
    fn command_adapter_forwards_lifecycle_and_input_to_the_runtime() {
        let path =
            std::env::temp_dir().join(format!("ped-37-command-adapter-{}.gb", std::process::id()));
        fs::write(&path, b"PED-37 command adapter ROM").expect("write synthetic ROM");
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));

        assert_eq!(
            runtime_snapshot_impl(&runtime).expect("snapshot").phase,
            RuntimePhase::Empty
        );
        assert_eq!(
            open_rom_impl(path.clone(), &runtime).expect("open").phase,
            RuntimePhase::Paused
        );
        set_keyboard_input_impl(vec![RuntimeButton::A], &runtime).expect("input");
        assert_eq!(
            start_rom_impl(&runtime).expect("start").phase,
            RuntimePhase::Running
        );
        assert_eq!(
            pause_rom_impl(&runtime).expect("pause").phase,
            RuntimePhase::Paused
        );
        assert_eq!(
            restart_rom_impl(&runtime).expect("restart").phase,
            RuntimePhase::Running
        );
        assert_eq!(
            close_rom_impl(&runtime).expect("close").phase,
            RuntimePhase::Empty
        );

        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn command_adapter_subscribe_returns_the_exact_snapshot() {
        let control_bodies = Arc::new(Mutex::new(Vec::new()));
        let control_capture = Arc::clone(&control_bodies);
        let events = Channel::<RuntimeEvent>::new(move |body| {
            control_capture.lock().expect("control bodies").push(body);
            Ok(())
        });
        let frames = Channel::<Response>::new(|_| Ok(()));
        let runtime = DesktopRuntime::spawn(Arc::new(ContractMockCoreFactory));

        let snapshot = subscribe_runtime_impl(events, frames, &runtime).expect("subscribe");

        assert_eq!(snapshot.phase, RuntimePhase::Empty);
        assert_eq!(control_bodies.lock().expect("control bodies").len(), 1);
        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn command_adapter_control_channel_is_json_and_frame_channel_is_raw() {
        let control_bodies = Arc::new(Mutex::new(Vec::new()));
        let control_capture = Arc::clone(&control_bodies);
        let events = Channel::<RuntimeEvent>::new(move |body| {
            control_capture.lock().expect("control bodies").push(body);
            Ok(())
        });
        let frame_bodies = Arc::new(Mutex::new(Vec::new()));
        let frame_capture = Arc::clone(&frame_bodies);
        let frames = Channel::<Response>::new(move |body| {
            frame_capture.lock().expect("frame bodies").push(body);
            Ok(())
        });
        let observer = ChannelObserver::new(events, frames);

        observer
            .publish_control(RuntimeEvent::Snapshot {
                snapshot: crate::emulator::contracts::RuntimeSnapshot::empty(),
            })
            .expect("publish control");
        observer
            .publish_frame(vec![1, 2, 3, 4])
            .expect("publish raw frame");

        let control = control_bodies.lock().expect("control bodies").pop();
        assert!(matches!(control, Some(InvokeResponseBody::Json(_))));
        let frame = frame_bodies.lock().expect("frame bodies").pop();
        assert!(matches!(frame, Some(InvokeResponseBody::Raw(bytes)) if bytes == [1, 2, 3, 4]));
    }
}
