use crate::apu::Apu;
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

impl AudioDevice for Apu {
    fn read(&self, address: u16) -> u8 {
        Apu::read(self, address)
    }

    fn write(&mut self, address: u16, value: u8) {
        Apu::write(self, address, value);
    }

    fn tick(&mut self, t_cycles: u32) -> TickEffects {
        for _ in 0..t_cycles {
            self.tick_t_cycle();
        }
        TickEffects::default()
    }

    fn stereo_frames_available(&self) -> usize {
        Apu::stereo_frames_available(self)
    }

    fn drain_audio(&mut self) -> AudioBatch {
        Apu::drain_audio(self)
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
