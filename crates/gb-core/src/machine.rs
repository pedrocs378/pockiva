use std::num::NonZeroU32;

use crate::bus::MachineBus;
use crate::cartridge::Cartridge;
use crate::cpu::Cpu;
use crate::joypad::InputMatrix;
use crate::{
    AudioBatch, BatteryState, CartridgeMetadata, Clock, CoreError, EmulatorCore, Frame,
    InputSourceId, JoypadState, RunOutcome,
};

pub struct GameBoy<C: Clock + Send> {
    clock: C,
    sample_rate: NonZeroU32,
    cpu: Cpu,
    bus: Option<MachineBus>,
    inputs: InputMatrix,
}

impl<C: Clock + Send> GameBoy<C> {
    #[must_use]
    pub fn new(clock: C, sample_rate: NonZeroU32) -> Self {
        Self {
            clock,
            sample_rate,
            cpu: Cpu::post_boot_dmg(),
            bus: None,
            inputs: InputMatrix::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn run_one_for_rom_test(&mut self) -> Result<(u32, bool), CoreError> {
        let bus = self.bus.as_mut().ok_or(CoreError::NotLoaded)?;
        bus.set_unix_seconds(self.clock.unix_seconds());
        let result = self.cpu.step(bus)?;
        Ok((result.t_cycles, result.debug_breakpoint))
    }

    #[cfg(test)]
    pub(crate) fn next_step_for_rom_test(&mut self) -> Result<u32, CoreError> {
        let bus = self.bus.as_mut().ok_or(CoreError::NotLoaded)?;
        bus.set_unix_seconds(self.clock.unix_seconds());
        self.cpu.next_step_t_cycles(bus)
    }

    #[cfg(test)]
    pub(crate) const fn diagnostic_registers(&self) -> [u8; 6] {
        self.cpu.diagnostic_registers()
    }

    #[cfg(test)]
    pub(crate) fn serial_output(&self) -> &[u8] {
        self.bus.as_ref().map_or(&[], MachineBus::serial_output)
    }
}

impl<C: Clock + Send> EmulatorCore for GameBoy<C> {
    fn load_rom(
        &mut self,
        rom: &[u8],
        persisted: Option<&BatteryState>,
    ) -> Result<CartridgeMetadata, CoreError> {
        let now = self.clock.unix_seconds();
        let cartridge = Cartridge::load(rom, persisted, now)?;
        let metadata = cartridge.metadata().clone();
        let mut bus = MachineBus::new(cartridge, self.sample_rate, now);
        bus.set_joypad_state(self.inputs.effective());
        self.cpu = Cpu::post_boot_dmg();
        self.bus = Some(bus);
        Ok(metadata)
    }

    fn reset(&mut self) -> Result<(), CoreError> {
        let bus = self.bus.as_mut().ok_or(CoreError::NotLoaded)?;
        bus.reset(self.clock.unix_seconds(), self.inputs.effective());
        self.cpu = Cpu::post_boot_dmg();
        Ok(())
    }

    fn run_cycles(&mut self, cycle_budget: u32) -> Result<RunOutcome, CoreError> {
        let bus = self.bus.as_mut().ok_or(CoreError::NotLoaded)?;
        let mut executed = 0;
        while executed < cycle_budget {
            bus.set_unix_seconds(self.clock.unix_seconds());
            let next = self.cpu.next_step_t_cycles(bus)?;
            if next == 0 || next > cycle_budget - executed {
                break;
            }
            let result = self.cpu.step(bus)?;
            debug_assert_eq!(result.t_cycles, next);
            executed += result.t_cycles;
        }
        Ok(RunOutcome::new(
            executed,
            bus.frame_ready(),
            bus.stereo_frames_available(),
        ))
    }

    fn set_input(&mut self, source: InputSourceId, state: JoypadState) {
        self.inputs.set(source, state);
        if let Some(bus) = self.bus.as_mut() {
            bus.set_joypad_state(self.inputs.effective());
        }
    }

    fn clear_input_source(&mut self, source: InputSourceId) {
        self.inputs.clear(source);
        if let Some(bus) = self.bus.as_mut() {
            bus.set_joypad_state(self.inputs.effective());
        }
    }

    fn take_frame(&mut self) -> Option<Frame> {
        self.bus.as_mut().and_then(MachineBus::take_frame)
    }

    fn drain_audio(&mut self) -> AudioBatch {
        self.bus.as_mut().map_or_else(
            || AudioBatch::empty(self.sample_rate),
            MachineBus::drain_audio,
        )
    }

    fn battery_state(&self) -> Option<BatteryState> {
        self.bus
            .as_ref()
            .and_then(|bus| bus.battery_state(self.clock.unix_seconds()))
    }
}
