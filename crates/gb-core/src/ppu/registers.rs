#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Lcdc(u8);

impl Lcdc {
    pub(crate) const fn post_boot_dmg() -> Self {
        Self(0x91)
    }

    pub(crate) const fn bits(self) -> u8 {
        self.0
    }

    pub(crate) const fn enabled(self) -> bool {
        self.0 & 0x80 != 0
    }

    pub(crate) const fn window_map_high(self) -> bool {
        self.0 & 0x40 != 0
    }

    pub(crate) const fn window_enabled(self) -> bool {
        self.0 & 0x20 != 0
    }

    pub(crate) const fn tile_data_unsigned(self) -> bool {
        self.0 & 0x10 != 0
    }

    pub(crate) const fn background_map_high(self) -> bool {
        self.0 & 0x08 != 0
    }

    pub(crate) const fn object_height(self) -> u8 {
        if self.0 & 0x04 != 0 { 16 } else { 8 }
    }

    pub(crate) const fn objects_enabled(self) -> bool {
        self.0 & 0x02 != 0
    }

    pub(crate) const fn background_enabled(self) -> bool {
        self.0 & 0x01 != 0
    }

    pub(crate) fn write(&mut self, value: u8) {
        self.0 = value;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Stat(u8);

impl Stat {
    pub(crate) const fn enables(self) -> u8 {
        self.0 & 0x78
    }

    pub(crate) fn write(&mut self, value: u8) {
        self.0 = value & 0x78;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Registers {
    pub(crate) lcdc: Lcdc,
    pub(crate) stat: Stat,
    pub(crate) scy: u8,
    pub(crate) scx: u8,
    pub(crate) lyc: u8,
    pub(crate) dma: u8,
    pub(crate) bgp: u8,
    pub(crate) obp0: u8,
    pub(crate) obp1: u8,
    pub(crate) wy: u8,
    pub(crate) wx: u8,
}

impl Registers {
    pub(crate) const fn post_boot_dmg() -> Self {
        Self {
            lcdc: Lcdc::post_boot_dmg(),
            stat: Stat(0),
            scy: 0,
            scx: 0,
            lyc: 0,
            dma: 0xff,
            bgp: 0xfc,
            obp0: 0xff,
            obp1: 0xff,
            wy: 0,
            wx: 0,
        }
    }
}
