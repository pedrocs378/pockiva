use std::num::NonZeroU32;

use crate::interrupts::InterruptMask;
use crate::{AudioBatch, Frame};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TickEffects {
    pub(crate) requested_interrupts: InterruptMask,
}

impl TickEffects {
    pub(crate) const fn union(self, other: Self) -> Self {
        Self {
            requested_interrupts: self.requested_interrupts.union(other.requested_interrupts),
        }
    }
}

pub(crate) trait VideoDevice: Send {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, value: u8) -> TickEffects;
    fn dma_write_oam(&mut self, index: u8, value: u8);
    fn tick(&mut self, t_cycles: u32) -> TickEffects;
    fn frame_ready(&self) -> bool;
    fn take_frame(&mut self) -> Option<Frame>;
}

pub(crate) trait AudioDevice: Send {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, value: u8);
    fn tick(&mut self, t_cycles: u32) -> TickEffects;
    fn stereo_frames_available(&self) -> usize;
    fn drain_audio(&mut self) -> AudioBatch;
}

pub(crate) struct AudioRegisters {
    registers: [u8; 0x30],
    sample_rate: NonZeroU32,
}

impl AudioRegisters {
    pub(crate) const fn new(sample_rate: NonZeroU32) -> Self {
        Self {
            registers: [0; 0x30],
            sample_rate,
        }
    }
}

impl AudioDevice for AudioRegisters {
    fn read(&self, address: u16) -> u8 {
        self.registers
            .get(usize::from(address.saturating_sub(0xff10)))
            .copied()
            .unwrap_or(0xff)
    }
    fn write(&mut self, address: u16, value: u8) {
        if let Some(register) = self
            .registers
            .get_mut(usize::from(address.saturating_sub(0xff10)))
        {
            *register = value;
        }
    }
    fn tick(&mut self, _t_cycles: u32) -> TickEffects {
        TickEffects::default()
    }
    fn stereo_frames_available(&self) -> usize {
        0
    }
    fn drain_audio(&mut self) -> AudioBatch {
        AudioBatch::empty(self.sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioDevice, VideoDevice};

    fn assert_send<T: Send>() {}

    #[test]
    fn peripheral_trait_objects_are_send() {
        assert_send::<Box<dyn VideoDevice>>();
        assert_send::<Box<dyn AudioDevice>>();
    }
}
