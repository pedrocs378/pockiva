#![forbid(unsafe_code)]

mod contracts;

pub use contracts::{FoundationStatus, foundation_status};

/// Starts the Tauri desktop shell.
///
/// # Panics
///
/// Panics when Tauri cannot initialize or run the application event loop.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![foundation_status])
        .run(tauri::generate_context!())
        .expect("error while running the Game Boy desktop application");
}
