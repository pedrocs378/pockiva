use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::time::Instant;

use gb_core::InputSourceId;

use crate::{
    ClientMessage, MAX_SAFE_SEQUENCE, ProtocolVersion, RejectionReason, Sequence, ServerMessage,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionValueError;

impl Display for SessionValueError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("session identifiers and tokens must not be empty")
    }
}

impl Error for SessionValueError {}

macro_rules! non_empty_identifier {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            /// Creates a validated identifier.
            ///
            /// # Errors
            ///
            /// Returns [`SessionValueError`] when `value` is empty.
            pub fn new(value: impl Into<String>) -> Result<Self, SessionValueError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(SessionValueError);
                }
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

non_empty_identifier!(SessionId);
non_empty_identifier!(ControllerConnectionId);

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SessionToken(String);

impl SessionToken {
    /// Creates a validated pairing token.
    ///
    /// # Errors
    ///
    /// Returns [`SessionValueError`] when `value` is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, SessionValueError> {
        let value = value.into();
        if value.is_empty() {
            return Err(SessionValueError);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Debug for SessionToken {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("SessionToken")
            .field(&"REDACTED")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControllerEvent {
    Connected {
        connection_id: ControllerConnectionId,
        input_source: InputSourceId,
    },
    Message {
        connection_id: ControllerConnectionId,
        input_source: InputSourceId,
        message: ClientMessage,
    },
    Disconnected {
        connection_id: ControllerConnectionId,
        input_source: InputSourceId,
    },
}

impl ControllerEvent {
    #[must_use]
    pub const fn input_source(&self) -> InputSourceId {
        match self {
            Self::Connected { input_source, .. }
            | Self::Message { input_source, .. }
            | Self::Disconnected { input_source, .. } => *input_source,
        }
    }
}

pub trait ControllerEventSink: Send + Sync {
    /// Publishes one authenticated controller event at the serialized runtime boundary.
    ///
    /// # Errors
    ///
    /// Returns [`ControllerEventSinkError`] when the consumer cannot accept the event.
    fn publish(
        &self,
        event: ControllerEvent,
        received_at: Instant,
    ) -> Result<(), ControllerEventSinkError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControllerEventSinkError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionAction {
    Connected {
        reply: ServerMessage,
        event: ControllerEvent,
    },
    Input(ControllerEvent),
    Reply(ServerMessage),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionError {
    rejection_reason: RejectionReason,
}

impl SessionError {
    const fn new(rejection_reason: RejectionReason) -> Self {
        Self { rejection_reason }
    }

    #[must_use]
    pub const fn rejection_reason(self) -> RejectionReason {
        self.rejection_reason
    }
}

impl Display for SessionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "controller session rejected: {:?}",
            self.rejection_reason
        )
    }
}

impl Error for SessionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthenticationState {
    AwaitingHello,
    Authenticated,
}

#[derive(Debug)]
pub struct SessionMachine {
    token: SessionToken,
    connection_id: ControllerConnectionId,
    input_source: InputSourceId,
    expires_at: Instant,
    authentication: AuthenticationState,
    last_sequence: Option<Sequence>,
    emitted_disconnect: bool,
}

impl SessionMachine {
    #[must_use]
    pub fn new(
        token: SessionToken,
        connection_id: ControllerConnectionId,
        input_source: InputSourceId,
        expires_at: Instant,
    ) -> Self {
        Self {
            token,
            connection_id,
            input_source,
            expires_at,
            authentication: AuthenticationState::AwaitingHello,
            last_sequence: None,
            emitted_disconnect: false,
        }
    }

    /// Authenticates the first protocol frame.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when the first frame is not a valid, timely hello for this
    /// session.
    pub fn accept_hello(
        &mut self,
        message: ClientMessage,
        received_at: Instant,
    ) -> Result<SessionAction, SessionError> {
        if self.authentication != AuthenticationState::AwaitingHello {
            return Err(SessionError::new(RejectionReason::MalformedMessage));
        }

        let ClientMessage::Hello { version, token } = message else {
            return Err(SessionError::new(RejectionReason::MalformedMessage));
        };
        if received_at >= self.expires_at || token != self.token.expose() {
            return Err(SessionError::new(RejectionReason::InvalidToken));
        }
        if version != ProtocolVersion::V1 {
            return Err(SessionError::new(RejectionReason::UnsupportedVersion));
        }

        self.authentication = AuthenticationState::Authenticated;
        let event = ControllerEvent::Connected {
            connection_id: self.connection_id.clone(),
            input_source: self.input_source,
        };
        Ok(SessionAction::Connected {
            reply: ServerMessage::Welcome {
                version: ProtocolVersion::V1,
                controller_id: self.connection_id.as_str().to_owned(),
            },
            event,
        })
    }

    /// Applies one authenticated protocol-v1 message.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] for pre-authentication messages, repeated hello frames, or
    /// non-contiguous sequences.
    pub fn apply(
        &mut self,
        message: ClientMessage,
        _received_at: Instant,
    ) -> Result<SessionAction, SessionError> {
        if self.authentication != AuthenticationState::Authenticated {
            return Err(SessionError::new(RejectionReason::MalformedMessage));
        }
        let Some(sequence) = message_sequence(&message) else {
            return Err(SessionError::new(RejectionReason::MalformedMessage));
        };
        if let Some(previous) = self.last_sequence {
            let expected = if previous.get() == MAX_SAFE_SEQUENCE {
                0
            } else {
                previous.get() + 1
            };
            if sequence.get() != expected {
                return Err(SessionError::new(RejectionReason::MalformedMessage));
            }
        }
        self.last_sequence = Some(sequence);

        match message {
            ClientMessage::Ping { sequence } => {
                Ok(SessionAction::Reply(ServerMessage::Pong { sequence }))
            }
            ClientMessage::ButtonDown { .. }
            | ClientMessage::ButtonUp { .. }
            | ClientMessage::StateSync { .. } => {
                Ok(SessionAction::Input(ControllerEvent::Message {
                    connection_id: self.connection_id.clone(),
                    input_source: self.input_source,
                    message,
                }))
            }
            ClientMessage::Hello { .. } => unreachable!("hello was rejected above"),
        }
    }

    #[must_use]
    pub fn disconnect(&mut self) -> Option<ControllerEvent> {
        if self.authentication != AuthenticationState::Authenticated || self.emitted_disconnect {
            return None;
        }
        self.emitted_disconnect = true;
        Some(ControllerEvent::Disconnected {
            connection_id: self.connection_id.clone(),
            input_source: self.input_source,
        })
    }
}

const fn message_sequence(message: &ClientMessage) -> Option<Sequence> {
    match message {
        ClientMessage::ButtonDown { sequence, .. }
        | ClientMessage::ButtonUp { sequence, .. }
        | ClientMessage::StateSync { sequence, .. }
        | ClientMessage::Ping { sequence } => Some(*sequence),
        ClientMessage::Hello { .. } => None,
    }
}
