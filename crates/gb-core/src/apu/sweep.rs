#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SweepTrigger {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SweepClock {
    Idle,
    Applied(u16),
    AppliedAndDisabled(u16),
    Disabled,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct FrequencySweep {
    register: u8,
    pace: u8,
    negate: bool,
    shift: u8,
    timer: u8,
    shadow_frequency: u16,
    enabled: bool,
    negate_used: bool,
}

impl FrequencySweep {
    pub(crate) fn write(&mut self, value: u8) -> bool {
        let disabling_negate = self.negate_used && self.negate && value & 0x08 == 0;
        self.register = value & 0x7f;
        self.pace = (value >> 4) & 0x07;
        self.negate = value & 0x08 != 0;
        self.shift = value & 0x07;
        disabling_negate
    }

    pub(crate) fn trigger(&mut self, frequency: u16) -> SweepTrigger {
        self.shadow_frequency = frequency;
        self.timer = self.reload_period();
        self.enabled = self.pace != 0 || self.shift != 0;
        self.negate_used = false;
        if self.shift != 0 {
            if self.calculate().is_none() {
                self.enabled = false;
                return SweepTrigger::Disabled;
            }
            if self.negate {
                self.negate_used = true;
            }
        }
        SweepTrigger::Enabled
    }

    pub(crate) fn clock(&mut self) -> SweepClock {
        if !self.enabled {
            return SweepClock::Idle;
        }
        self.timer = self.timer.saturating_sub(1);
        if self.timer != 0 {
            return SweepClock::Idle;
        }
        self.timer = self.reload_period();
        if self.pace == 0 {
            return SweepClock::Idle;
        }
        let Some(frequency) = self.calculate() else {
            self.enabled = false;
            return SweepClock::Disabled;
        };
        if self.negate {
            self.negate_used = true;
        }
        if self.shift == 0 {
            return SweepClock::Idle;
        }
        self.shadow_frequency = frequency;
        if self.calculate().is_none() {
            self.enabled = false;
            SweepClock::AppliedAndDisabled(frequency)
        } else {
            SweepClock::Applied(frequency)
        }
    }

    fn calculate(&self) -> Option<u16> {
        let delta = self.shadow_frequency >> self.shift;
        let result = if self.negate {
            self.shadow_frequency.checked_sub(delta)?
        } else {
            self.shadow_frequency.checked_add(delta)?
        };
        (result <= 2047).then_some(result)
    }

    const fn reload_period(self) -> u8 {
        if self.pace == 0 { 8 } else { self.pace }
    }

    pub(crate) const fn register(self) -> u8 {
        self.register
    }
}
