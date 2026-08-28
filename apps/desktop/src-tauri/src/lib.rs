#![forbid(unsafe_code)]

use std::num::NonZeroU32;
use std::sync::Arc;

use audio::{AudioOutputFactory, CpalAudioOutputFactory};
use emulator::commands::{
    acknowledge_frame, close_rom, open_rom, pause_rom, restart_rom, runtime_snapshot,
    set_keyboard_input, start_rom, subscribe_runtime,
};
use emulator::factory::GameBoyCoreFactory;
use emulator::runtime::DesktopRuntime;
use gb_network::NetworkError;
use remote::commands::{
    end_remote_session, remote_snapshot, start_remote_session, subscribe_remote,
};
use remote::manager::RemoteSessionManager;
use tauri::Manager;

mod audio;
mod contracts;
mod emulator;
mod remote;
mod video;

pub use contracts::{FoundationStatus, foundation_status};

/// Starts the Tauri desktop shell.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the application event loop.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let controller_assets = app.path().resource_dir()?.join("controller");
            if !controller_assets.join("index.html").is_file() {
                return Err(NetworkError::AssetsUnavailable.into());
            }

            let audio_factory: Arc<dyn AudioOutputFactory> = Arc::new(CpalAudioOutputFactory);
            let prepared_output = audio_factory.open_default();
            let runtime_sample_rate = prepared_output.as_ref().map_or_else(
                |_| NonZeroU32::new(48_000).expect("48 kHz is non-zero"),
                |output| output.sample_rate(),
            );
            let core_factory = Arc::new(GameBoyCoreFactory::new(runtime_sample_rate));
            let runtime = DesktopRuntime::spawn_with_audio_preflight(
                core_factory,
                audio_factory,
                runtime_sample_rate,
                prepared_output,
            );
            let remote = RemoteSessionManager::new(runtime.handle(), controller_assets);
            app.manage(runtime);
            app.manage(remote);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            foundation_status,
            runtime_snapshot,
            subscribe_runtime,
            open_rom,
            start_rom,
            pause_rom,
            restart_rom,
            close_rom,
            set_keyboard_input,
            acknowledge_frame,
            remote_snapshot,
            subscribe_remote,
            start_remote_session,
            end_remote_session,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the Game Boy desktop application");

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            let _ = app_handle.state::<RemoteSessionManager>().shutdown();
            let _ = app_handle.state::<DesktopRuntime>().shutdown();
        }
    });
}
