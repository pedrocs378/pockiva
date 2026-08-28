//! Bounded desktop audio output and pacing policy.

use std::num::NonZeroU32;
use std::time::Duration;

use gb_core::AudioBatch;

mod cpal_backend;
mod policy;
mod queue;

pub(crate) use cpal_backend::CpalAudioOutputFactory;
pub(crate) use policy::{AudioWatermarks, pacing_decision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PacingDecision {
    Prime,
    RunOneBatch,
    Wait(Duration),
    Backpressured(Duration),
    FallbackFrameDeadline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AudioBackendErrorKind {
    NoOutputDevice,
    UnsupportedConfiguration,
    DeviceUnavailable,
    DeviceBusy,
    PermissionDenied,
    StreamInvalidated,
    Backend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AudioBackendError {
    pub(crate) kind: AudioBackendErrorKind,
    pub(crate) message: String,
}

impl AudioBackendError {
    pub(crate) fn new(kind: AudioBackendErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AudioHealth {
    pub(crate) queued_stereo_frames: usize,
    pub(crate) flush_pending: bool,
    pub(crate) underruns: u64,
    pub(crate) dropped_stereo_frames: u64,
    pub(crate) stream_errors: u64,
    pub(crate) usable: bool,
}

pub(crate) trait AudioOutput: Send {
    fn sample_rate(&self) -> NonZeroU32;
    fn watermarks(&self) -> AudioWatermarks;
    fn enqueue(&mut self, batch: &AudioBatch) -> Result<(), AudioBackendError>;
    fn set_gain(&mut self, gain: f32) -> Result<(), AudioBackendError>;
    fn health(&self) -> AudioHealth;
    fn play(&mut self) -> Result<(), AudioBackendError>;
    fn pause_and_flush(&mut self) -> Result<(), AudioBackendError>;
    fn shutdown(&mut self);
}

pub(crate) trait AudioOutputFactory: Send + Sync {
    fn open_default(&self) -> Result<Box<dyn AudioOutput>, AudioBackendError>;
}

#[cfg(test)]
mod tests;
