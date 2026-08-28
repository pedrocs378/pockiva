#![forbid(unsafe_code)]

mod bus;
mod cartridge;
mod cpu;
mod interrupts;

pub mod contracts;

pub use contracts::{
    AudioBatch, BatteryState, Button, CartridgeMetadata, Clock, CompatibilityMode, CoreError,
    EmulatorCore, Frame, InputSourceId, JoypadState, MapperKind, RunOutcome, SCREEN_HEIGHT,
    SCREEN_WIDTH,
};

pub const DMG_CLOCK_HZ: u32 = 4_194_304;
pub const T_CYCLES_PER_M_CYCLE: u32 = 4;
