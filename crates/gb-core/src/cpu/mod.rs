mod decode;
mod execute;
mod registers;

use crate::CoreError;
use crate::interrupts::{Interrupt, InterruptMask};
use registers::Registers;

pub(crate) use execute::StepResult;

pub(crate) trait CpuBus {
    fn read8(&mut self, address: u16) -> u8;
    fn write8(&mut self, address: u16, value: u8);
    fn idle_m_cycle(&mut self);
    fn peek8(&self, address: u16) -> u8;
    fn elapsed_t_cycles(&self) -> u64;
    fn pending_interrupts(&self) -> InterruptMask;
    fn acknowledge_interrupt(&mut self, interrupt: Interrupt);
    fn reset_divider(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CpuMode {
    Running,
    Halted,
    Stopped,
}

pub(crate) struct Cpu {
    pub(crate) registers: Registers,
    ime: bool,
    ime_enable_delay: u8,
    mode: CpuMode,
    halt_bug: bool,
}

impl Cpu {
    pub(crate) const fn post_boot_dmg() -> Self {
        Self {
            registers: Registers::post_boot_dmg(),
            ime: false,
            ime_enable_delay: 0,
            mode: CpuMode::Running,
            halt_bug: false,
        }
    }

    pub(crate) fn next_step_t_cycles(&self, bus: &impl CpuBus) -> Result<u32, CoreError> {
        execute::next_step_t_cycles(self, bus)
    }

    pub(crate) fn step(&mut self, bus: &mut impl CpuBus) -> Result<StepResult, CoreError> {
        execute::step(self, bus)
    }

    #[cfg(test)]
    pub(crate) const fn diagnostic_registers(&self) -> [u8; 6] {
        [
            self.registers.b,
            self.registers.c,
            self.registers.d,
            self.registers.e,
            self.registers.h,
            self.registers.l,
        ]
    }
}

#[cfg(test)]
mod tests;
