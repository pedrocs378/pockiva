#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Button {
    Up = 0,
    Down = 1,
    Left = 2,
    Right = 3,
    A = 4,
    B = 5,
    Start = 6,
    Select = 7,
}

impl Button {
    const fn mask(self) -> u8 {
        1 << (self as u8)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct JoypadState(u8);

impl JoypadState {
    pub fn press(&mut self, button: Button) {
        self.0 |= button.mask();
    }

    pub fn release(&mut self, button: Button) {
        self.0 &= !button.mask();
    }

    #[must_use]
    pub const fn is_pressed(self, button: Button) -> bool {
        self.0 & button.mask() != 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputSourceId(u64);

impl InputSourceId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}
