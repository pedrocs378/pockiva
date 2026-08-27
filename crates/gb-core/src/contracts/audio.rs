use super::CoreError;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioBatch {
    sample_rate: u32,
    interleaved_stereo: Vec<f32>,
}

impl AudioBatch {
    #[must_use]
    pub const fn empty(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            interleaved_stereo: Vec::new(),
        }
    }

    /// Creates a stereo audio batch.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InternalInvariant`] when the sample rate is zero or
    /// the interleaved sample buffer does not contain complete stereo pairs.
    pub fn new(sample_rate: u32, interleaved_stereo: Vec<f32>) -> Result<Self, CoreError> {
        if sample_rate == 0 {
            return Err(CoreError::InternalInvariant(
                "audio sample rate must be non-zero".into(),
            ));
        }
        if !interleaved_stereo.len().is_multiple_of(2) {
            return Err(CoreError::InternalInvariant(
                "stereo audio must contain complete left/right pairs".into(),
            ));
        }

        Ok(Self {
            sample_rate,
            interleaved_stereo,
        })
    }

    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    #[must_use]
    pub fn samples(&self) -> &[f32] {
        &self.interleaved_stereo
    }

    #[must_use]
    pub fn stereo_frame_count(&self) -> usize {
        self.interleaved_stereo.len() / 2
    }
}
