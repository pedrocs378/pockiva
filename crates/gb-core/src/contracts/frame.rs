use super::CoreError;

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;
const RGBA_CHANNELS: usize = 4;
const FRAME_BYTE_LEN: usize = SCREEN_WIDTH * SCREEN_HEIGHT * RGBA_CHANNELS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    sequence: u64,
    rgba: Box<[u8]>,
}

impl Frame {
    #[must_use]
    pub fn blank() -> Self {
        Self {
            sequence: 0,
            rgba: vec![0; FRAME_BYTE_LEN].into_boxed_slice(),
        }
    }

    /// Creates a frame with a fixed DMG-sized RGBA buffer.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InternalInvariant`] when `rgba` is not exactly
    /// `160 * 144 * 4` bytes long.
    pub fn new(sequence: u64, rgba: Vec<u8>) -> Result<Self, CoreError> {
        if rgba.len() != FRAME_BYTE_LEN {
            return Err(CoreError::InternalInvariant(format!(
                "frame must contain {FRAME_BYTE_LEN} RGBA bytes, received {}",
                rgba.len()
            )));
        }

        Ok(Self {
            sequence,
            rgba: rgba.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}
