#![forbid(unsafe_code)]

mod message;
mod rate_limit;
mod server;
mod session;

pub use message::{
    Button, ClientMessage, MAX_SAFE_SEQUENCE, PROTOCOL_VERSION, ProtocolVersion, RejectionReason,
    Sequence, ServerMessage,
};
pub use rate_limit::InputRateLimiter;
pub use server::{
    ControllerServer, NetworkError, OsSessionEntropy, PairingInfo, SessionEntropy,
    SessionServerConfig, discover_lan_ipv4,
};
pub use session::{
    ControllerConnectionId, ControllerEvent, ControllerEventSink, ControllerEventSinkError,
    SessionAction, SessionError, SessionId, SessionMachine, SessionToken, SessionValueError,
};
