use std::num::NonZeroU32;

use gb_core::{
    AudioBatch, BatteryState, Button, CartridgeMetadata, Clock, CoreError, EmulatorCore, Frame,
    GameBoy, InputSourceId, JoypadState, RunOutcome, SCREEN_HEIGHT, SCREEN_WIDTH,
};

struct FixedClock(u64);

impl Clock for FixedClock {
    fn unix_seconds(&self) -> u64 {
        self.0
    }
}

#[derive(Default)]
struct FakeCore {
    input: JoypadState,
}

impl EmulatorCore for FakeCore {
    fn load_rom(
        &mut self,
        _rom: &[u8],
        _persisted: Option<&BatteryState>,
    ) -> Result<CartridgeMetadata, CoreError> {
        Err(CoreError::InvalidRom(
            "fixture is intentionally empty".into(),
        ))
    }

    fn reset(&mut self) -> Result<(), CoreError> {
        Ok(())
    }

    fn run_cycles(&mut self, cycle_budget: u32) -> Result<RunOutcome, CoreError> {
        Ok(RunOutcome::idle(cycle_budget))
    }

    fn set_input(&mut self, _source: InputSourceId, state: JoypadState) {
        self.input = state;
    }

    fn clear_input_source(&mut self, _source: InputSourceId) {
        self.input = JoypadState::default();
    }

    fn take_frame(&mut self) -> Option<Frame> {
        None
    }

    fn drain_audio(&mut self) -> AudioBatch {
        AudioBatch::empty(NonZeroU32::new(48_000).expect("sample rate is non-zero"))
    }

    fn battery_state(&self) -> Option<BatteryState> {
        None
    }
}

#[test]
fn frame_dimensions_are_fixed_to_dmg_output() {
    assert_eq!(SCREEN_WIDTH, 160);
    assert_eq!(SCREEN_HEIGHT, 144);
    assert_eq!(Frame::blank().rgba().len(), 160 * 144 * 4);
}

#[test]
fn joypad_tracks_only_valid_buttons() {
    let mut input = JoypadState::default();
    input.press(Button::A);
    input.press(Button::Left);
    assert!(input.is_pressed(Button::A));
    assert!(input.is_pressed(Button::Left));
    input.release(Button::A);
    assert!(!input.is_pressed(Button::A));
}

#[test]
fn clock_and_emulator_contracts_are_implementable() {
    let clock = FixedClock(1_234);
    assert_eq!(clock.unix_seconds(), 1_234);

    let mut core = FakeCore::default();
    let outcome = core.run_cycles(456).expect("fake core can run");
    assert_eq!(outcome.cycles_executed(), 456);
}

#[test]
fn audio_batch_exposes_a_non_zero_sample_rate() {
    let sample_rate = NonZeroU32::new(48_000).expect("sample rate is non-zero");
    let batch = AudioBatch::empty(sample_rate);

    assert_eq!(batch.sample_rate(), sample_rate);
}

fn assert_emulator_thread_bound<T: EmulatorCore + Send>() {}

#[test]
fn concrete_core_satisfies_the_runtime_thread_contract() {
    assert_emulator_thread_bound::<GameBoy<FixedClock>>();
}
