use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

use gb_core::InputSourceId;

use crate::ClientMessage;

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
            Self::Message { input_source, .. } | Self::Disconnected { input_source, .. } => {
                *input_source
            }
        }
    }
}
