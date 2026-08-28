#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Flags(u8);

impl Flags {
    pub(crate) const Z: Self = Self(0x80);
    pub(crate) const N: Self = Self(0x40);
    pub(crate) const H: Self = Self(0x20);
    pub(crate) const C: Self = Self(0x10);

    pub(crate) const fn from_bits(bits: u8) -> Self {
        Self(bits & 0xf0)
    }
    pub(crate) const fn bits(self) -> u8 {
        self.0
    }
    pub(crate) const fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
    pub(crate) const fn union(self, other: Self) -> Self {
        Self::from_bits(self.0 | other.0)
    }
    pub(crate) fn set(&mut self, flag: Self, enabled: bool) {
        self.0 = if enabled {
            self.0 | flag.0
        } else {
            self.0 & !flag.0
        };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Registers {
    pub(crate) a: u8,
    pub(crate) f: Flags,
    pub(crate) b: u8,
    pub(crate) c: u8,
    pub(crate) d: u8,
    pub(crate) e: u8,
    pub(crate) h: u8,
    pub(crate) l: u8,
    pub(crate) sp: u16,
    pub(crate) pc: u16,
}

impl Registers {
    pub(crate) const fn post_boot_dmg() -> Self {
        Self {
            a: 0x01,
            f: Flags::from_bits(0xb0),
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xd8,
            h: 0x01,
            l: 0x4d,
            sp: 0xfffe,
            pc: 0x0100,
        }
    }
    pub(crate) const fn af(self) -> u16 {
        u16::from_be_bytes([self.a, self.f.bits()])
    }
    pub(crate) const fn bc(self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }
    pub(crate) const fn de(self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }
    pub(crate) const fn hl(self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }
    pub(crate) fn set_af(&mut self, value: u16) {
        let [a, f] = value.to_be_bytes();
        self.a = a;
        self.f = Flags::from_bits(f);
    }
    pub(crate) fn set_bc(&mut self, value: u16) {
        [self.b, self.c] = value.to_be_bytes();
    }
    pub(crate) fn set_de(&mut self, value: u16) {
        [self.d, self.e] = value.to_be_bytes();
    }
    pub(crate) fn set_hl(&mut self, value: u16) {
        [self.h, self.l] = value.to_be_bytes();
    }
}

pub(crate) fn add8(left: u8, right: u8, carry: bool) -> (u8, Flags) {
    let carry_value = u8::from(carry);
    let (first, c1) = left.overflowing_add(right);
    let (result, c2) = first.overflowing_add(carry_value);
    let mut flags = Flags::default();
    flags.set(Flags::Z, result == 0);
    flags.set(
        Flags::H,
        (left & 0x0f) + (right & 0x0f) + carry_value > 0x0f,
    );
    flags.set(Flags::C, c1 || c2);
    (result, flags)
}

pub(crate) fn sub8(left: u8, right: u8, carry: bool) -> (u8, Flags) {
    let carry_value = u8::from(carry);
    let result = left.wrapping_sub(right).wrapping_sub(carry_value);
    let mut flags = Flags::N;
    flags.set(Flags::Z, result == 0);
    flags.set(Flags::H, (left & 0x0f) < (right & 0x0f) + carry_value);
    flags.set(
        Flags::C,
        u16::from(left) < u16::from(right) + u16::from(carry_value),
    );
    (result, flags)
}

pub(crate) fn inc8(value: u8, carry: bool) -> (u8, Flags) {
    let result = value.wrapping_add(1);
    let mut flags = if carry { Flags::C } else { Flags::default() };
    flags.set(Flags::Z, result == 0);
    flags.set(Flags::H, value & 0x0f == 0x0f);
    (result, flags)
}

pub(crate) fn dec8(value: u8, carry: bool) -> (u8, Flags) {
    let result = value.wrapping_sub(1);
    let mut flags = Flags::N;
    flags.set(Flags::C, carry);
    flags.set(Flags::Z, result == 0);
    flags.set(Flags::H, value & 0x0f == 0);
    (result, flags)
}

pub(crate) fn daa(a: u8, flags: Flags) -> (u8, Flags) {
    let mut correction = 0;
    let mut carry = flags.contains(Flags::C);
    if flags.contains(Flags::H) || (!flags.contains(Flags::N) && a & 0x0f > 9) {
        correction |= 0x06;
    }
    if carry || (!flags.contains(Flags::N) && a > 0x99) {
        correction |= 0x60;
        carry = true;
    }
    let result = if flags.contains(Flags::N) {
        a.wrapping_sub(correction)
    } else {
        a.wrapping_add(correction)
    };
    let mut output = Flags::default();
    output.set(Flags::Z, result == 0);
    output.set(Flags::N, flags.contains(Flags::N));
    output.set(Flags::C, carry);
    (result, output)
}
