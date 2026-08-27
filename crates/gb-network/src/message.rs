use std::collections::HashSet;

use serde::de::Error as DeserializeError;
use serde::{Deserialize, Deserializer, Serialize};

pub const PROTOCOL_VERSION: &str = "v1";

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
        sequence: u64,
    },
    ButtonUp {
        button: Button,
        sequence: u64,
    },
    StateSync {
        #[serde(deserialize_with = "deserialize_unique_buttons")]
        buttons: Vec<Button>,
        sequence: u64,
    },
    Ping {
        sequence: u64,
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
        sequence: u64,
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
