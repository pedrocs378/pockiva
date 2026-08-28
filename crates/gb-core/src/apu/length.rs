#[derive(Debug, Clone, Copy)]
pub(crate) struct LengthCounter<const MAX: u16> {
    remaining: u16,
    enabled: bool,
}

impl<const MAX: u16> Default for LengthCounter<MAX> {
    fn default() -> Self {
        Self {
            remaining: 0,
            enabled: false,
        }
    }
}

impl<const MAX: u16> LengthCounter<MAX> {
    pub(crate) fn load(&mut self, raw: u8) {
        let value = u16::from(raw) & (MAX - 1);
        self.remaining = MAX - value;
    }

    pub(crate) fn trigger(&mut self, next_step_clocks_length: bool) -> bool {
        if self.remaining == 0 {
            self.remaining = MAX;
            if self.enabled && !next_step_clocks_length {
                self.remaining -= 1;
            }
        }
        self.remaining == 0
    }

    pub(crate) fn set_enabled(
        &mut self,
        enabled: bool,
        next_step_clocks_length: bool,
        channel_active: bool,
    ) -> bool {
        if !self.enabled && enabled && !next_step_clocks_length && self.remaining > 0 {
            self.remaining -= 1;
        }
        self.enabled = enabled;
        channel_active && self.enabled && self.remaining == 0
    }

    pub(crate) fn clock(&mut self) -> bool {
        if !self.enabled || self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        self.remaining == 0
    }

    #[cfg(test)]
    pub(crate) const fn remaining(self) -> u16 {
        self.remaining
    }

    pub(crate) const fn enabled(self) -> bool {
        self.enabled
    }
}
