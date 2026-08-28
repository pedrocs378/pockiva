#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct OamDma {
    source_high: u8,
    index: u8,
    active: bool,
}

impl OamDma {
    pub(crate) fn start(&mut self, source_high: u8) {
        self.source_high = source_high;
        self.index = 0;
        self.active = true;
    }
    pub(crate) const fn active(self) -> bool {
        self.active
    }
    pub(crate) fn next_address(self) -> Option<(u16, u8)> {
        if self.active {
            Some((
                (u16::from(self.source_high) << 8) | u16::from(self.index),
                self.index,
            ))
        } else {
            None
        }
    }
    pub(crate) fn advance(&mut self) {
        self.index = self.index.wrapping_add(1);
        if self.index == 160 {
            self.active = false;
        }
    }
}
