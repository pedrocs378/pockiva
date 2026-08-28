//! Deterministic contract stand-in kept in production through PED-37.
//! PED-39 alone replaces its registration with a real `GameBoy` and platform `SystemClock`.

use std::collections::HashMap;
use std::num::NonZeroU32;

use gb_core::{
    AudioBatch, BatteryState, CartridgeMetadata, CompatibilityMode, CoreError, EmulatorCore, Frame,
    InputSourceId, JoypadState, MapperKind, RunOutcome, SCREEN_HEIGHT, SCREEN_WIDTH,
};

use super::runtime::{CoreFactory, RuntimeCore};

#[derive(Debug, Default)]
pub struct ContractMockCore {
    loaded: bool,
    sequence: u64,
    frame: Option<Frame>,
    inputs: HashMap<InputSourceId, JoypadState>,
}

impl ContractMockCore {
    fn diagnostic_frame(&self) -> Result<Frame, CoreError> {
        const PALETTE: [[u8; 4]; 4] = [
            [224, 248, 208, 255],
            [136, 192, 112, 255],
            [52, 104, 86, 255],
            [8, 24, 32, 255],
        ];
        let shift = usize::try_from(self.sequence % 4).expect("modulo four fits usize");
        let mut rgba = Vec::with_capacity(SCREEN_WIDTH * SCREEN_HEIGHT * 4);
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let color = PALETTE[((x / 20) + (y / 18) + shift) % PALETTE.len()];
                rgba.extend_from_slice(&color);
            }
        }
        Frame::new(self.sequence, rgba)
    }

    #[cfg(test)]
    fn input_for(&self, source: InputSourceId) -> Option<JoypadState> {
        self.inputs.get(&source).copied()
    }
}

impl EmulatorCore for ContractMockCore {
    fn load_rom(
        &mut self,
        rom: &[u8],
        _persisted: Option<&BatteryState>,
    ) -> Result<CartridgeMetadata, CoreError> {
        if rom.is_empty() {
            return Err(CoreError::InvalidRom("ROM file is empty".into()));
        }
        self.loaded = true;
        self.sequence = 0;
        self.frame = None;
        self.inputs.clear();
        Ok(CartridgeMetadata {
            title: "PED-37 Desktop Preview".into(),
            rom_identity: "ped-37-contract-mock".into(),
            mapper: MapperKind::RomOnly,
            compatibility: CompatibilityMode::Dmg,
            ram_size_bytes: 0,
            has_battery: false,
        })
    }

    fn reset(&mut self) -> Result<(), CoreError> {
        if !self.loaded {
            return Err(CoreError::NotLoaded);
        }
        self.sequence = 0;
        self.frame = None;
        Ok(())
    }

    fn run_cycles(&mut self, cycle_budget: u32) -> Result<RunOutcome, CoreError> {
        if !self.loaded {
            return Err(CoreError::NotLoaded);
        }
        self.sequence = self.sequence.wrapping_add(1);
        self.frame = Some(self.diagnostic_frame()?);
        Ok(RunOutcome::new(cycle_budget, true, 0))
    }

    fn set_input(&mut self, source: InputSourceId, state: JoypadState) {
        self.inputs.insert(source, state);
    }

    fn clear_input_source(&mut self, source: InputSourceId) {
        self.inputs.remove(&source);
    }

    fn take_frame(&mut self) -> Option<Frame> {
        self.frame.take()
    }

    fn drain_audio(&mut self) -> AudioBatch {
        AudioBatch::empty(NonZeroU32::new(48_000).expect("48 kHz is non-zero"))
    }

    fn battery_state(&self) -> Option<BatteryState> {
        None
    }
}

#[derive(Debug, Default)]
pub struct ContractMockCoreFactory;

impl CoreFactory for ContractMockCoreFactory {
    fn create(&self) -> Box<dyn RuntimeCore> {
        Box::new(ContractMockCore::default())
    }
}

#[cfg(test)]
mod tests {
    use gb_core::{Button, CoreError, EmulatorCore, InputSourceId, JoypadState};

    use super::ContractMockCore;

    #[test]
    fn contract_mock_rejects_only_empty_rom_bytes() {
        let mut core = ContractMockCore::default();
        assert!(matches!(
            core.load_rom(&[], None),
            Err(CoreError::InvalidRom(_))
        ));

        let metadata = core
            .load_rom(b"PED-37 synthetic ROM", None)
            .expect("non-empty synthetic bytes load");
        assert_eq!(metadata.title, "PED-37 Desktop Preview");
        assert_eq!(metadata.rom_identity, "ped-37-contract-mock");
        assert_eq!(metadata.ram_size_bytes, 0);
        assert!(!metadata.has_battery);
    }

    #[test]
    fn contract_mock_requires_a_loaded_rom_for_reset() {
        let mut core = ContractMockCore::default();
        assert_eq!(core.reset(), Err(CoreError::NotLoaded));
        core.load_rom(b"fixture", None).expect("loads");
        core.reset().expect("loaded mock resets");
    }

    #[test]
    fn contract_mock_preserves_source_aware_input() {
        let mut core = ContractMockCore::default();
        core.load_rom(b"fixture", None).expect("loads");
        let mut state = JoypadState::default();
        state.press(Button::Left);
        state.press(Button::A);
        core.set_input(InputSourceId::new(1), state);
        assert_eq!(core.input_for(InputSourceId::new(1)), Some(state));
        core.clear_input_source(InputSourceId::new(1));
        assert_eq!(core.input_for(InputSourceId::new(1)), None);
    }

    #[test]
    fn contract_mock_produces_monotonic_frames_and_empty_audio() {
        let mut core = ContractMockCore::default();
        core.load_rom(b"fixture", None).expect("loads");

        let first = core.run_cycles(70_224).expect("runs");
        assert!(first.frame_ready());
        assert_eq!(first.cycles_executed(), 70_224);
        let first_frame = core.take_frame().expect("frame one");

        core.run_cycles(70_224).expect("runs again");
        let second_frame = core.take_frame().expect("frame two");
        assert_eq!(first_frame.sequence(), 1);
        assert_eq!(second_frame.sequence(), 2);
        assert_ne!(first_frame.rgba(), second_frame.rgba());

        let audio = core.drain_audio();
        assert_eq!(audio.sample_rate().get(), 48_000);
        assert_eq!(audio.stereo_frame_count(), 0);
        assert!(core.battery_state().is_none());
    }
}
