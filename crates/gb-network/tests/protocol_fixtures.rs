use std::fs;
use std::path::PathBuf;

use gb_network::{ClientMessage, ServerMessage};
use serde_json::Value;

fn fixtures() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/protocol/fixtures/protocol-v1.json");
    let contents = fs::read_to_string(path).expect("canonical protocol fixture must be readable");
    serde_json::from_str(&contents).expect("canonical protocol fixture must be valid JSON")
}

#[test]
fn rust_accepts_and_round_trips_every_typescript_fixture() {
    let fixtures = fixtures();

    for value in fixtures["validClientMessages"]
        .as_array()
        .expect("client fixture array")
    {
        let message: ClientMessage =
            serde_json::from_value(value.clone()).expect("valid client fixture");
        assert_eq!(
            serde_json::to_value(message).expect("serialize client"),
            *value
        );
    }

    for value in fixtures["validServerMessages"]
        .as_array()
        .expect("server fixture array")
    {
        let message: ServerMessage =
            serde_json::from_value(value.clone()).expect("valid server fixture");
        assert_eq!(
            serde_json::to_value(message).expect("serialize server"),
            *value
        );
    }
}

#[test]
fn rust_rejects_every_invalid_typescript_fixture() {
    let fixtures = fixtures();

    for value in fixtures["invalidMessages"]
        .as_array()
        .expect("invalid fixture array")
    {
        assert!(serde_json::from_value::<ClientMessage>(value.clone()).is_err());
        assert!(serde_json::from_value::<ServerMessage>(value.clone()).is_err());
    }
}
