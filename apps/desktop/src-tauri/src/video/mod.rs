//! Raw desktop video transport.

mod packet;
mod queue;

pub(crate) use packet::encode_frame_packet;
pub(crate) use queue::{AcknowledgeError, FrameQueue};

#[cfg(test)]
mod tests;
