#![forbid(unsafe_code)]

pub mod contracts;

pub use contracts::{
    AudioBatch, BatteryState, Button, CartridgeMetadata, Clock, CompatibilityMode, CoreError,
    EmulatorCore, Frame, InputSourceId, JoypadState, MapperKind, RunOutcome, SCREEN_HEIGHT,
    SCREEN_WIDTH,
};
