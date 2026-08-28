#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct VolumeEnvelope {
    register: u8,
    initial_volume: u8,
    increase: bool,
    pace: u8,
    timer: u8,
    current_volume: u8,
    running: bool,
}

impl VolumeEnvelope {
    #[cfg(test)]
    pub(crate) fn from_register(value: u8) -> Self {
        let mut envelope = Self::default();
        envelope.write(value);
        envelope
    }

    pub(crate) fn write(&mut self, value: u8) {
        self.register = value;
        self.initial_volume = value >> 4;
        self.increase = value & 0x08 != 0;
        self.pace = value & 0x07;
    }

    pub(crate) fn trigger(&mut self) {
        self.current_volume = self.initial_volume;
        self.timer = self.reload_period();
        self.running = true;
    }

    pub(crate) fn clock(&mut self) {
        if !self.running {
            return;
        }
        self.timer = self.timer.saturating_sub(1);
        if self.timer != 0 {
            return;
        }
        self.timer = self.reload_period();
        let next = if self.increase {
            self.current_volume
                .checked_add(1)
                .filter(|value| *value <= 15)
        } else {
            self.current_volume.checked_sub(1)
        };
        if let Some(next) = next {
            self.current_volume = next;
        } else {
            self.running = false;
        }
    }

    const fn reload_period(self) -> u8 {
        if self.pace == 0 { 8 } else { self.pace }
    }

    pub(crate) const fn register(self) -> u8 {
        self.register
    }

    pub(crate) const fn volume(self) -> u8 {
        self.current_volume
    }

    pub(crate) const fn dac_enabled(self) -> bool {
        self.register & 0xf8 != 0
    }
}
