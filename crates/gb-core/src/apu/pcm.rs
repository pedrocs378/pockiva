use std::collections::VecDeque;
use std::num::NonZeroU32;

use crate::AudioBatch;

pub(crate) const MAX_CORE_STEREO_FRAMES: usize = 4_096;

#[derive(Debug, Clone)]
pub(crate) struct PcmBuffer {
    samples: VecDeque<f32>,
    dropped_stereo_frames: u64,
}

impl Default for PcmBuffer {
    fn default() -> Self {
        Self {
            samples: VecDeque::with_capacity(MAX_CORE_STEREO_FRAMES * 2),
            dropped_stereo_frames: 0,
        }
    }
}

impl PcmBuffer {
    pub(crate) fn push_stereo(&mut self, left: f32, right: f32) {
        if self.stereo_frames_available() == MAX_CORE_STEREO_FRAMES {
            let _ = self.samples.pop_front();
            let _ = self.samples.pop_front();
            self.dropped_stereo_frames += 1;
        }
        self.samples.push_back(left);
        self.samples.push_back(right);
    }

    pub(crate) fn stereo_frames_available(&self) -> usize {
        self.samples.len() / 2
    }

    pub(crate) fn drain(&mut self, sample_rate: NonZeroU32) -> AudioBatch {
        if self.samples.is_empty() {
            return AudioBatch::empty(sample_rate);
        }
        let samples = self.samples.drain(..).collect();
        AudioBatch::new(sample_rate, samples).expect("PCM buffer contains complete stereo pairs")
    }

    pub(crate) fn clear(&mut self) {
        self.samples.clear();
        self.dropped_stereo_frames = 0;
    }

    #[cfg(test)]
    pub(crate) const fn dropped_stereo_frames(&self) -> u64 {
        self.dropped_stereo_frames
    }
}
