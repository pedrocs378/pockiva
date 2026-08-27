use std::error::Error;
use std::fmt::{Display, Formatter};

use super::{AudioBatch, BatteryState, CartridgeMetadata, Frame, InputSourceId, JoypadState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    InvalidRom(String),
    UnsupportedCgbOnlyCartridge,
    UnsupportedMapper(u8),
    NotLoaded,
    InternalInvariant(String),
}

impl Display for CoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRom(reason) => write!(formatter, "invalid ROM: {reason}"),
            Self::UnsupportedCgbOnlyCartridge => {
                formatter.write_str("CGB-only cartridges are unsupported")
            }
            Self::UnsupportedMapper(mapper) => {
                write!(formatter, "unsupported cartridge mapper: {mapper:#04x}")
            }
            Self::NotLoaded => formatter.write_str("no cartridge is loaded"),
            Self::InternalInvariant(reason) => {
                write!(formatter, "internal invariant violation: {reason}")
            }
        }
    }
}

impl Error for CoreError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOutcome {
    cycles_executed: u32,
    frame_ready: bool,
    stereo_frames_available: usize,
}

impl RunOutcome {
    #[must_use]
    pub const fn new(
        cycles_executed: u32,
        frame_ready: bool,
        stereo_frames_available: usize,
    ) -> Self {
        Self {
            cycles_executed,
            frame_ready,
            stereo_frames_available,
        }
    }

    #[must_use]
    pub const fn idle(cycles_executed: u32) -> Self {
        Self::new(cycles_executed, false, 0)
    }

    #[must_use]
    pub const fn cycles_executed(self) -> u32 {
        self.cycles_executed
    }

    #[must_use]
    pub const fn frame_ready(self) -> bool {
        self.frame_ready
    }

    #[must_use]
    pub const fn stereo_frames_available(self) -> usize {
        self.stereo_frames_available
    }
}

pub trait EmulatorCore {
    /// Loads and validates a cartridge image with optional persisted state.
    ///
    /// # Errors
    ///
    /// Returns a typed [`CoreError`] when the ROM or persisted state cannot be accepted.
    fn load_rom(
        &mut self,
        rom: &[u8],
        persisted: Option<&BatteryState>,
    ) -> Result<CartridgeMetadata, CoreError>;
    /// Restores the currently loaded cartridge to its initial execution state.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::NotLoaded`] when no cartridge is loaded or another
    /// typed error when the core cannot restore a valid state.
    fn reset(&mut self) -> Result<(), CoreError>;
    /// Advances all clocked subsystems by at most the supplied cycle budget.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::NotLoaded`] when no cartridge is loaded or another
    /// typed error when advancement violates a core invariant.
    fn run_cycles(&mut self, cycle_budget: u32) -> Result<RunOutcome, CoreError>;
    fn set_input(&mut self, source: InputSourceId, state: JoypadState);
    fn clear_input_source(&mut self, source: InputSourceId);
    fn take_frame(&mut self) -> Option<Frame>;
    fn drain_audio(&mut self) -> AudioBatch;
    fn battery_state(&self) -> Option<BatteryState>;
}
