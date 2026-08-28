use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RemotePhase {
    Off,
    Waiting,
    Connected,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RemoteErrorCode {
    NoLanAddress,
    BindFailed,
    AssetsUnavailable,
    ServerFailed,
    RuntimeUnavailable,
    InvalidLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteError {
    pub code: RemoteErrorCode,
    pub message: String,
}

impl RemoteError {
    pub(crate) fn new(code: RemoteErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteLatency {
    pub samples: u64,
    pub last_ms: u64,
    pub p95_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteSnapshot {
    pub phase: RemotePhase,
    pub pairing_url: Option<String>,
    pub expires_at_unix_ms: Option<u64>,
    pub controller_id: Option<String>,
    pub latency: Option<RemoteLatency>,
    pub error: Option<RemoteError>,
}

impl RemoteSnapshot {
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn off() -> Self {
        Self {
            phase: RemotePhase::Off,
            pairing_url: None,
            expires_at_unix_ms: None,
            controller_id: None,
            latency: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(crate) enum RemoteEvent {
    Snapshot { snapshot: RemoteSnapshot },
}

pub(crate) type RemoteResult<T> = Result<T, RemoteError>;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        RemoteError, RemoteErrorCode, RemoteEvent, RemoteLatency, RemotePhase, RemoteSnapshot,
    };

    #[test]
    fn remote_snapshots_and_events_use_the_exact_frontend_shape() {
        let snapshot = RemoteSnapshot {
            phase: RemotePhase::Connected,
            pairing_url: Some("http://192.0.2.2:1234/?token=redacted".to_owned()),
            expires_at_unix_ms: Some(123_456),
            controller_id: Some("controller-1".to_owned()),
            latency: Some(RemoteLatency {
                samples: 3,
                last_ms: 7,
                p95_ms: 9,
            }),
            error: None,
        };

        assert_eq!(
            serde_json::to_value(RemoteEvent::Snapshot { snapshot })
                .expect("remote event serializes"),
            json!({
                "type": "snapshot",
                "snapshot": {
                    "phase": "connected",
                    "pairingUrl": "http://192.0.2.2:1234/?token=redacted",
                    "expiresAtUnixMs": 123_456,
                    "controllerId": "controller-1",
                    "latency": { "samples": 3, "lastMs": 7, "p95Ms": 9 },
                    "error": null
                }
            })
        );
    }

    #[test]
    fn remote_error_codes_are_kebab_case_and_off_contains_no_pairing_secret() {
        assert_eq!(
            serde_json::to_value(RemoteSnapshot::off()).expect("off serializes"),
            json!({
                "phase": "off",
                "pairingUrl": null,
                "expiresAtUnixMs": null,
                "controllerId": null,
                "latency": null,
                "error": null
            })
        );
        assert_eq!(
            serde_json::to_value(RemoteError::new(
                RemoteErrorCode::NoLanAddress,
                "No LAN address."
            ))
            .expect("error serializes"),
            json!({ "code": "no-lan-address", "message": "No LAN address." })
        );
        assert_eq!(
            serde_json::to_value([
                RemoteErrorCode::BindFailed,
                RemoteErrorCode::AssetsUnavailable,
                RemoteErrorCode::ServerFailed,
                RemoteErrorCode::RuntimeUnavailable,
                RemoteErrorCode::InvalidLifecycle,
            ])
            .expect("codes serialize"),
            json!([
                "bind-failed",
                "assets-unavailable",
                "server-failed",
                "runtime-unavailable",
                "invalid-lifecycle"
            ])
        );
    }
}
