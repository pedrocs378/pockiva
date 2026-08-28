use std::time::{Duration, Instant};

use gb_core::InputSourceId;
use gb_network::{
    Button, ClientMessage, ControllerConnectionId, ControllerEvent, InputRateLimiter,
    MAX_SAFE_SEQUENCE, ProtocolVersion, RejectionReason, Sequence, ServerMessage, SessionAction,
    SessionId, SessionMachine, SessionToken,
};

fn connection_id() -> ControllerConnectionId {
    ControllerConnectionId::new("controller-1").expect("connection id")
}

fn token() -> SessionToken {
    SessionToken::new("super-secret").expect("session token")
}

fn hello(token: &str) -> ClientMessage {
    ClientMessage::Hello {
        version: ProtocolVersion::V1,
        token: token.to_owned(),
    }
}

fn state_sync(sequence: u64) -> ClientMessage {
    ClientMessage::StateSync {
        buttons: vec![Button::Left, Button::A],
        sequence: Sequence::new(sequence).expect("safe sequence"),
    }
}

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

#[test]
fn authenticated_session_accepts_contiguous_input_and_ping_then_cleans_up_once() {
    let now = Instant::now();
    let connection_id = connection_id();
    let input_source = InputSourceId::new(2);
    let mut machine = SessionMachine::new(
        token(),
        connection_id.clone(),
        input_source,
        now + Duration::from_secs(600),
    );

    assert!(matches!(
        machine.accept_hello(hello("super-secret"), now),
        Ok(SessionAction::Connected {
            reply: ServerMessage::Welcome {
                version: ProtocolVersion::V1,
                ..
            },
            event: ControllerEvent::Connected {
                input_source: connected_source,
                ..
            },
        }) if connected_source == input_source
    ));
    assert!(matches!(
        machine.apply(state_sync(41), now),
        Ok(SessionAction::Input(ControllerEvent::Message { .. }))
    ));
    assert!(matches!(
        machine.apply(
            ClientMessage::ButtonDown {
                button: Button::Start,
                sequence: Sequence::new(42).expect("sequence"),
            },
            now,
        ),
        Ok(SessionAction::Input(ControllerEvent::Message { .. }))
    ));
    assert!(matches!(
        machine.apply(
            ClientMessage::ButtonUp {
                button: Button::Start,
                sequence: Sequence::new(43).expect("sequence"),
            },
            now,
        ),
        Ok(SessionAction::Input(ControllerEvent::Message { .. }))
    ));
    assert!(matches!(
        machine.apply(
            ClientMessage::Ping {
                sequence: Sequence::new(44).expect("sequence"),
            },
            now,
        ),
        Ok(SessionAction::Reply(ServerMessage::Pong { sequence })) if sequence.get() == 44
    ));
    assert_eq!(
        machine.disconnect(),
        Some(ControllerEvent::Disconnected {
            connection_id,
            input_source,
        })
    );
    assert_eq!(machine.disconnect(), None);
}

#[test]
fn session_rejects_invalid_or_expired_authentication_and_non_hello_first_frames() {
    let now = Instant::now();
    let expires_at = now + Duration::from_secs(1);

    for (message, received_at, expected) in [
        (hello("wrong"), now, RejectionReason::InvalidToken),
        (
            hello("super-secret"),
            expires_at,
            RejectionReason::InvalidToken,
        ),
        (state_sync(0), now, RejectionReason::MalformedMessage),
    ] {
        let mut machine =
            SessionMachine::new(token(), connection_id(), InputSourceId::new(2), expires_at);
        let error = machine
            .accept_hello(message, received_at)
            .expect_err("authentication must fail");
        assert_eq!(error.rejection_reason(), expected);
        assert_eq!(machine.disconnect(), None);
    }
}

#[test]
fn authenticated_session_rejects_second_hello_and_non_contiguous_sequences() {
    let now = Instant::now();
    let mut machine = SessionMachine::new(
        token(),
        connection_id(),
        InputSourceId::new(2),
        now + Duration::from_secs(600),
    );
    machine
        .accept_hello(hello("super-secret"), now)
        .expect("hello accepted");

    assert_eq!(
        machine
            .apply(hello("super-secret"), now)
            .expect_err("second hello rejected")
            .rejection_reason(),
        RejectionReason::MalformedMessage
    );
    assert!(machine.apply(state_sync(50), now).is_ok());
    for invalid in [50, 52] {
        assert_eq!(
            machine
                .apply(state_sync(invalid), now)
                .expect_err("sequence rejected")
                .rejection_reason(),
            RejectionReason::MalformedMessage
        );
    }
}

#[test]
fn sequence_wraps_only_from_maximum_safe_integer_to_zero() {
    let now = Instant::now();
    let mut machine = SessionMachine::new(
        token(),
        connection_id(),
        InputSourceId::new(2),
        now + Duration::from_secs(600),
    );
    machine
        .accept_hello(hello("super-secret"), now)
        .expect("hello accepted");

    machine
        .apply(state_sync(MAX_SAFE_SEQUENCE), now)
        .expect("first sequence may be any safe integer");
    machine
        .apply(state_sync(0), now)
        .expect("maximum wraps to zero");
    assert_eq!(
        machine
            .apply(state_sync(0), now)
            .expect_err("zero may not repeat")
            .rejection_reason(),
        RejectionReason::MalformedMessage
    );
}

#[test]
fn deterministic_rate_limiter_refills_without_floating_point() {
    let start = Instant::now();
    let mut limiter = InputRateLimiter::new(240, 64, start);

    for offset in 0..64 {
        assert!(limiter.allow(start + Duration::from_micros(offset)));
    }
    assert!(!limiter.allow(start + Duration::from_millis(1)));
    assert!(limiter.allow(start + Duration::from_millis(5)));
}

#[test]
fn rate_limiter_clamps_backward_time_and_never_exceeds_capacity() {
    let start = Instant::now();
    let mut limiter = InputRateLimiter::new(1, 2, start);

    assert!(limiter.allow(start));
    assert!(limiter.allow(start));
    let before_start = start
        .checked_sub(Duration::from_secs(1))
        .expect("test instant supports one-second subtraction");
    assert!(!limiter.allow(before_start));
    assert!(limiter.allow(start + Duration::from_secs(100)));
    assert!(limiter.allow(start + Duration::from_secs(100)));
    assert!(!limiter.allow(start + Duration::from_secs(100)));
}
