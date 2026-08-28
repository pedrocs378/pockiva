use super::envelope::VolumeEnvelope;
use super::length::LengthCounter;
use super::sweep::{FrequencySweep, SweepClock, SweepTrigger};

const DUTY: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

#[derive(Debug, Clone)]
pub(crate) struct PulseChannel {
    has_sweep: bool,
    enabled: bool,
    dac_enabled: bool,
    duty: u8,
    duty_position: u8,
    frequency: u16,
    timer: u16,
    length: LengthCounter<64>,
    envelope: VolumeEnvelope,
    sweep: Option<FrequencySweep>,
}

impl PulseChannel {
    pub(crate) fn new(has_sweep: bool) -> Self {
        Self {
            has_sweep,
            enabled: false,
            dac_enabled: false,
            duty: 0,
            duty_position: 0,
            frequency: 0,
            timer: 0,
            length: LengthCounter::default(),
            envelope: VolumeEnvelope::default(),
            sweep: has_sweep.then(FrequencySweep::default),
        }
    }

    pub(crate) fn read(&self, register: u8) -> u8 {
        match register {
            0 if self.has_sweep => self
                .sweep
                .as_ref()
                .map_or(0xff, |sweep| sweep.register() | 0x80),
            1 => (self.duty << 6) | 0x3f,
            2 => self.envelope.register(),
            4 => 0xbf | (u8::from(self.length.enabled()) << 6),
            _ => 0xff,
        }
    }

    pub(crate) fn write(&mut self, register: u8, value: u8, next_step_clocks_length: bool) {
        match register {
            0 if self.has_sweep => {
                if self.sweep.as_mut().is_some_and(|sweep| sweep.write(value)) {
                    self.enabled = false;
                }
            }
            1 => {
                self.duty = value >> 6;
                self.length.load(value & 0x3f);
            }
            2 => {
                self.envelope.write(value);
                self.dac_enabled = self.envelope.dac_enabled();
                if !self.dac_enabled {
                    self.enabled = false;
                }
            }
            3 => self.frequency = (self.frequency & 0x0700) | u16::from(value),
            4 => {
                self.frequency = (self.frequency & 0x00ff) | (u16::from(value & 0x07) << 8);
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

    pub(crate) fn write_length_while_powered_off(&mut self, value: u8) {
        self.length.load(value & 0x3f);
    }

    fn trigger(&mut self, next_step_clocks_length: bool) {
        if self.length.trigger(next_step_clocks_length) {
            self.enabled = false;
        }
        self.timer = self.period();
        self.envelope.trigger();
        self.enabled = self.dac_enabled;
        if let Some(sweep) = &mut self.sweep
            && sweep.trigger(self.frequency) == SweepTrigger::Disabled
        {
            self.enabled = false;
        }
    }

    pub(crate) fn tick_t_cycle(&mut self) {
        self.timer = self.timer.saturating_sub(1);
        if self.timer == 0 {
            self.timer = self.period();
            self.duty_position = (self.duty_position + 1) & 7;
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

    pub(crate) fn clock_sweep(&mut self) {
        let Some(sweep) = &mut self.sweep else { return };
        match sweep.clock() {
            SweepClock::Applied(frequency) => self.frequency = frequency,
            SweepClock::AppliedAndDisabled(frequency) => {
                self.frequency = frequency;
                self.enabled = false;
            }
            SweepClock::Disabled => self.enabled = false,
            SweepClock::Idle => {}
        }
    }

    pub(crate) const fn output(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            0
        } else {
            DUTY[self.duty as usize][self.duty_position as usize] * self.envelope.volume()
        }
    }

    const fn period(&self) -> u16 {
        (2048 - self.frequency) * 4
    }

    pub(crate) const fn active(&self) -> bool {
        self.enabled
    }

    pub(crate) const fn dac_enabled(&self) -> bool {
        self.dac_enabled
    }

    pub(crate) fn power_off(&mut self) {
        *self = Self::new(self.has_sweep);
    }
}
