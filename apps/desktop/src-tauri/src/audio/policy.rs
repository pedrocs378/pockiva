use std::num::NonZeroU32;
use std::time::Duration;

use super::{AudioHealth, PacingDecision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AudioWatermarks {
    pub(crate) sample_rate: NonZeroU32,
    pub(crate) low: usize,
    pub(crate) target: usize,
    pub(crate) high: usize,
    pub(crate) capacity: usize,
}

impl AudioWatermarks {
    pub(crate) fn for_rate(sample_rate: NonZeroU32) -> Self {
        let frames = |milliseconds: u64| {
            usize::try_from(u64::from(sample_rate.get()) * milliseconds / 1_000)
                .expect("audio watermark fits usize")
        };
        let marks = Self {
            sample_rate,
            low: frames(40),
            target: frames(80),
            high: frames(120),
            capacity: frames(160),
        };
        assert!(
            marks.low < marks.target && marks.target < marks.high && marks.high < marks.capacity
        );
        marks
    }

    pub(crate) const fn sample_capacity(&self) -> usize {
        self.capacity * 2
    }
}

pub(crate) fn pacing_decision(health: AudioHealth, marks: AudioWatermarks) -> PacingDecision {
    if !health.usable {
        return PacingDecision::FallbackFrameDeadline;
    }
    let queued = health.queued_stereo_frames;
    if queued < marks.low {
        return PacingDecision::Prime;
    }
    if queued <= marks.target {
        return PacingDecision::RunOneBatch;
    }
    let excess = queued - marks.target;
    let numerator = u64::try_from(excess).expect("frame count fits u64") * 1_000_000_000;
    let denominator = u64::from(marks.sample_rate.get());
    let nanos = numerator.div_ceil(denominator).min(5_000_000);
    let wait = Duration::from_nanos(nanos);
    if queued > marks.high {
        PacingDecision::Backpressured(wait)
    } else {
        PacingDecision::Wait(wait)
    }
}
