use gb_core::InputSourceId;
use gb_network::{ControllerConnectionId, ControllerEvent, SessionId, SessionToken};

#[test]
fn identifiers_and_tokens_must_not_be_empty() {
    assert!(SessionId::new("").is_err());
    assert!(SessionToken::new("").is_err());
    assert!(ControllerConnectionId::new("").is_err());
}

#[test]
fn session_token_debug_output_is_redacted() {
    let token = SessionToken::new("super-secret").expect("non-empty token");
    let debug = format!("{token:?}");

    assert!(!debug.contains("super-secret"));
    assert!(debug.contains("REDACTED"));
}

#[test]
fn disconnect_event_identifies_the_input_source_to_clear() {
    let input_source = InputSourceId::new(42);
    let event = ControllerEvent::Disconnected {
        connection_id: ControllerConnectionId::new("controller-1").expect("connection id"),
        input_source,
    };

    assert_eq!(event.input_source(), input_source);
}
