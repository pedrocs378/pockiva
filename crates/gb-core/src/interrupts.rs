#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Interrupt {
    VBlank,
    LcdStat,
    Timer,
    Serial,
    Joypad,
}

impl Interrupt {
    pub(crate) const fn bit(self) -> u8 {
        match self {
            Self::VBlank => 0x01,
            Self::LcdStat => 0x02,
            Self::Timer => 0x04,
            Self::Serial => 0x08,
            Self::Joypad => 0x10,
        }
    }

    pub(crate) const fn vector(self) -> u16 {
        match self {
            Self::VBlank => 0x0040,
            Self::LcdStat => 0x0048,
            Self::Timer => 0x0050,
            Self::Serial => 0x0058,
            Self::Joypad => 0x0060,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct InterruptMask(u8);

impl InterruptMask {
    pub(crate) const fn from_bits(bits: u8) -> Self {
        Self(bits & 0x1f)
    }

    pub(crate) const fn bits(self) -> u8 {
        self.0
    }

    pub(crate) const fn union(self, other: Self) -> Self {
        Self::from_bits(self.0 | other.0)
    }

    pub(crate) const fn intersection(self, other: Self) -> Self {
        Self::from_bits(self.0 & other.0)
    }

    pub(crate) const fn without(self, other: Self) -> Self {
        Self::from_bits(self.0 & !other.0)
    }

    pub(crate) fn highest_priority(self) -> Option<Interrupt> {
        [
            Interrupt::VBlank,
            Interrupt::LcdStat,
            Interrupt::Timer,
            Interrupt::Serial,
            Interrupt::Joypad,
        ]
        .into_iter()
        .find(|interrupt| self.0 & interrupt.bit() != 0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct InterruptRegisters {
    enable: InterruptMask,
    flags: InterruptMask,
}

impl InterruptRegisters {
    pub(crate) const fn read_if(self) -> u8 {
        0xe0 | self.flags.bits()
    }
    pub(crate) const fn read_ie(self) -> u8 {
        self.enable.bits()
    }
    pub(crate) fn write_if(&mut self, value: u8) {
        self.flags = InterruptMask::from_bits(value);
    }
    pub(crate) fn write_ie(&mut self, value: u8) {
        self.enable = InterruptMask::from_bits(value);
    }
    pub(crate) fn request(&mut self, mask: InterruptMask) {
        self.flags = self.flags.union(mask);
    }
    pub(crate) const fn pending(self) -> InterruptMask {
        self.enable.intersection(self.flags)
    }
    pub(crate) fn acknowledge(&mut self, interrupt: Interrupt) {
        self.flags = self
            .flags
            .without(InterruptMask::from_bits(interrupt.bit()));
    }
}

#[cfg(test)]
mod tests {
    use super::{Interrupt, InterruptMask};
    use crate::{DMG_CLOCK_HZ, T_CYCLES_PER_M_CYCLE};

    #[test]
    fn pending_interrupts_choose_the_lowest_enabled_bit() {
        let pending = InterruptMask::from_bits(0b0001_1100);
        assert_eq!(pending.highest_priority(), Some(Interrupt::Timer));
        assert_eq!(Interrupt::Timer.vector(), 0x0050);
    }

    #[test]
    fn machine_cycle_is_four_t_cycles() {
        assert_eq!(T_CYCLES_PER_M_CYCLE, 4);
        assert_eq!(DMG_CLOCK_HZ, 4_194_304);
    }
}
