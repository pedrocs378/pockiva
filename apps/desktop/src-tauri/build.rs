fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "foundation_status",
            "runtime_snapshot",
            "subscribe_runtime",
            "open_rom",
            "start_rom",
            "pause_rom",
            "restart_rom",
            "close_rom",
            "set_keyboard_input",
            "acknowledge_frame",
        ]),
    ))
    .expect("failed to build Tauri application metadata");
}
