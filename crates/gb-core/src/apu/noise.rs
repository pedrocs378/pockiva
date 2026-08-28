use super::envelope::VolumeEnvelope;
use super::length::LengthCounter;

const DIVISORS: [u32; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

pub(crate) const fn noise_period(divisor_code: u8, shift: u8) -> u32 {
    DIVISORS[divisor_code as usize] << shift
}

#[derive(Debug, Default, Clone)]
pub(crate) struct NoiseChannel {
    enabled: bool,
    dac_enabled: bool,
    lfsr: u16,
    timer: u32,
    divisor_code: u8,
    shift: u8,
    width_mode: bool,
    length: LengthCounter<64>,
    envelope: VolumeEnvelope,
}

impl NoiseChannel {
    pub(crate) fn read(&self, register: u8) -> u8 {
        match register {
            1 => self.envelope.register(),
            2 => (self.shift << 4) | (u8::from(self.width_mode) << 3) | self.divisor_code,
            3 => 0xbf | (u8::from(self.length.enabled()) << 6),
            _ => 0xff,
        }
    }

    pub(crate) fn write(&mut self, register: u8, value: u8, next_step_clocks_length: bool) {
        match register {
            0 => self.length.load(value & 0x3f),
            1 => {
                self.envelope.write(value);
                self.dac_enabled = self.envelope.dac_enabled();
                if !self.dac_enabled {
                    self.enabled = false;
                }
            }
            2 => {
                self.shift = value >> 4;
                self.width_mode = value & 0x08 != 0;
                self.divisor_code = value & 0x07;
            }
            3 => {
                if self
                    .length
                    .set_enabled(value & 0x40 != 0, next_step_clocks_length, self.enabled)
                {
                    self.enabled = false;
                }
                if value & 0x80 != 0 {
                    self.trigger(next_step_clocks_length);
                }
            }
            _ => {}
        }
    }

    fn trigger(&mut self, next_step_clocks_length: bool) {
        let _ = self.length.trigger(next_step_clocks_length);
        self.lfsr = 0x7fff;
        self.timer = self.period();
        self.envelope.trigger();
        self.enabled = self.dac_enabled;
    }

    pub(crate) fn tick_t_cycle(&mut self) {
        self.timer = self.timer.saturating_sub(1);
        if self.timer == 0 {
            self.timer = self.period();
            let xor = (self.lfsr ^ (self.lfsr >> 1)) & 1;
            self.lfsr = (self.lfsr >> 1) | (xor << 14);
            if self.width_mode {
                self.lfsr = (self.lfsr & !(1 << 6)) | (xor << 6);
            }
        }
    }

    pub(crate) fn clock_length(&mut self) {
        if self.length.clock() {
            self.enabled = false;
        }
    }
    pub(crate) fn clock_envelope(&mut self) {
        self.envelope.clock();
    }
    pub(crate) const fn output(&self) -> u8 {
        if self.enabled && self.dac_enabled && self.lfsr & 1 == 0 {
            self.envelope.volume()
        } else {
            0
        }
    }
    const fn period(&self) -> u32 {
        noise_period(self.divisor_code, self.shift)
    }
    pub(crate) const fn active(&self) -> bool {
        self.enabled
    }
    pub(crate) const fn dac_enabled(&self) -> bool {
        self.dac_enabled
    }
    pub(crate) fn power_off(&mut self) {
        *self = Self::default();
    }
}
