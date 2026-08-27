use gb_core::{SCREEN_HEIGHT, SCREEN_WIDTH};
use gb_network::PROTOCOL_VERSION;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FoundationStatus {
    protocol_version: &'static str,
    screen_width: usize,
    screen_height: usize,
    remote_controller_limit: usize,
}

#[tauri::command]
#[must_use]
pub const fn foundation_status() -> FoundationStatus {
    FoundationStatus {
        protocol_version: PROTOCOL_VERSION,
        screen_width: SCREEN_WIDTH,
        screen_height: SCREEN_HEIGHT,
        remote_controller_limit: 1,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::foundation_status;

    #[test]
    fn foundation_status_serializes_frozen_contract_values() {
        let status =
            serde_json::to_value(foundation_status()).expect("foundation status serializes");

        assert_eq!(
            status,
            json!({
                "protocol_version": "v1",
                "screen_width": 160,
                "screen_height": 144,
                "remote_controller_limit": 1
            })
        );
    }
}
