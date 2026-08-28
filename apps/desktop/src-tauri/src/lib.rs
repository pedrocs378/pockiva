#![forbid(unsafe_code)]

use std::num::NonZeroU32;
use std::sync::Arc;

use audio::{AudioOutputFactory, CpalAudioOutputFactory};
use emulator::commands::{
    acknowledge_frame, close_rom, open_rom, pause_rom, restart_rom, runtime_snapshot,
    set_keyboard_input, start_rom, subscribe_runtime,
};
use emulator::mock_core::ContractMockCoreFactory;
use emulator::runtime::DesktopRuntime;
use tauri::Manager;

mod audio;
mod contracts;
mod emulator;
mod video;

pub use contracts::{FoundationStatus, foundation_status};

/// Starts the Tauri desktop shell.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the application event loop.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let audio_factory: Arc<dyn AudioOutputFactory> = Arc::new(CpalAudioOutputFactory);
    let prepared_output = audio_factory.open_default();
    let runtime_sample_rate = prepared_output.as_ref().map_or_else(
        |_| NonZeroU32::new(48_000).expect("48 kHz is non-zero"),
        |output| output.sample_rate(),
    );
    let core_factory = Arc::new(ContractMockCoreFactory::with_sample_rate(
        runtime_sample_rate,
    ));
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        // PED-39 owns replacing this contract mock with the SystemClock + GameBoy factory.
        .manage(DesktopRuntime::spawn_with_audio_preflight(
            core_factory,
            audio_factory,
            runtime_sample_rate,
            prepared_output,
        ))
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
        ])
        .build(tauri::generate_context!())
        .expect("error while building the Game Boy desktop application");

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            let _ = app_handle.state::<DesktopRuntime>().shutdown();
        }
    });
}
