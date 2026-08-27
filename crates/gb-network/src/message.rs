use std::collections::HashSet;

use serde::de::Error as DeserializeError;
use serde::{Deserialize, Deserializer, Serialize};

pub const PROTOCOL_VERSION: &str = "v1";
pub const MAX_SAFE_SEQUENCE: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Sequence(u64);

impl Sequence {
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value <= MAX_SAFE_SEQUENCE {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for Sequence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        Self::new(value).ok_or_else(|| {
            D::Error::custom(format_args!(
                "sequence must be at most JavaScript Number.MAX_SAFE_INTEGER ({MAX_SAFE_SEQUENCE})"
            ))
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolVersion {
    #[serde(rename = "v1")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Start,
    Select,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientMessage {
    Hello {
        version: ProtocolVersion,
        #[serde(deserialize_with = "deserialize_non_empty_string")]
        token: String,
    },
    ButtonDown {
        button: Button,
        sequence: Sequence,
    },
    ButtonUp {
        button: Button,
        sequence: Sequence,
    },
    StateSync {
        #[serde(deserialize_with = "deserialize_unique_buttons")]
        buttons: Vec<Button>,
        sequence: Sequence,
    },
    Ping {
        sequence: Sequence,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RejectionReason {
    InvalidToken,
    UnsupportedVersion,
    ControllerAlreadyConnected,
    MalformedMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ServerMessage {
    Welcome {
        version: ProtocolVersion,
        #[serde(deserialize_with = "deserialize_non_empty_string")]
        controller_id: String,
    },
    Rejected {
        reason: RejectionReason,
    },
    Pong {
        sequence: Sequence,
    },
    ControllerDisconnected,
}

fn deserialize_non_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.is_empty() {
        return Err(D::Error::custom("value must not be empty"));
    }
    Ok(value)
}

fn deserialize_unique_buttons<'de, D>(deserializer: D) -> Result<Vec<Button>, D::Error>
where
    D: Deserializer<'de>,
{
    let buttons = Vec::<Button>::deserialize(deserializer)?;
    let unique = buttons.iter().copied().collect::<HashSet<_>>();
    if buttons.len() != unique.len() {
        return Err(D::Error::custom("state-sync buttons must be unique"));
    }
    Ok(buttons)
}
