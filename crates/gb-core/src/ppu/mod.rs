//! DMG PPU implementation.

mod registers;
mod render;
mod sprite;
mod timing;

#[cfg(test)]
mod rom_tests;
#[cfg(test)]
mod tests;

use crate::bus::devices::{TickEffects, VideoDevice};
use crate::{Frame, SCREEN_HEIGHT, SCREEN_WIDTH};

use registers::Registers;
use render::RenderedRow;
use sprite::SelectedSprite;

pub(crate) const DOTS_PER_LINE: u16 = 456;
pub(crate) const VISIBLE_LINES: u8 = 144;
pub(crate) const LINES_PER_FRAME: u8 = 154;
pub(crate) const MODE2_DOTS: u16 = 80;
pub(crate) const STARTUP_MODE3_DOT: u16 = 84;
pub(crate) const BASE_MODE3_DOTS: u16 = 172;
pub(crate) const FRAME_RGBA_LEN: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 4;

pub(crate) const DMG_SHADES: [[u8; 4]; 4] = [
    [0xff, 0xff, 0xff, 0xff],
    [0xaa, 0xaa, 0xaa, 0xff],
    [0x55, 0x55, 0x55, 0xff],
    [0x00, 0x00, 0x00, 0xff],
];

pub(crate) fn map_palette(register: u8, color_number: u8) -> [u8; 4] {
    DMG_SHADES[usize::from((register >> (color_number * 2)) & 0x03)]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LcdMode {
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Drawing = 3,
}

#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Ppu {
    vram: Box<[u8; 0x2000]>,
    oam: Box<[u8; 0xa0]>,
    registers: Registers,
    mode: LcdMode,
    dot: u16,
    internal_line: u8,
    visible_hblank_dot: u16,
    mode3_end_dot: u16,
    stat_line_high: bool,
    lyc_equal: bool,
    startup_line: bool,
    lcd_phase: u16,
    window_line: u8,
    blank_first_frame: bool,
    framebuffer: Vec<u8>,
    pending_frame: Option<Frame>,
    next_sequence: u64,
    selected_sprites: Vec<SelectedSprite>,
    last_rendered_row: Option<RenderedRow>,
}

impl Ppu {
    pub(crate) fn post_boot_dmg() -> Self {
        Self {
            vram: Box::new([0; 0x2000]),
            oam: Box::new([0; 0xa0]),
            registers: Registers::post_boot_dmg(),
            mode: LcdMode::OamScan,
            dot: 0,
            internal_line: 0,
            visible_hblank_dot: MODE2_DOTS + BASE_MODE3_DOTS,
            mode3_end_dot: MODE2_DOTS + BASE_MODE3_DOTS,
            stat_line_high: false,
            lyc_equal: true,
            startup_line: false,
            lcd_phase: 0,
            window_line: 0,
            blank_first_frame: false,
            framebuffer: vec![0xff; FRAME_RGBA_LEN],
            pending_frame: None,
            next_sequence: 1,
            selected_sprites: Vec::new(),
            last_rendered_row: None,
        }
    }

    pub(crate) fn read(&self, address: u16) -> u8 {
        match address {
            0x8000..=0x9fff if self.vram_cpu_accessible() => {
                self.vram[usize::from(address - 0x8000)]
            }
            0xfe00..=0xfe9f if self.oam_cpu_accessible() => self.oam[usize::from(address - 0xfe00)],
            0xff40 => self.registers.lcdc.bits(),
            0xff41 => {
                0x80 | self.registers.stat.enables()
                    | u8::from(self.lyc_equal) << 2
                    | self.mode as u8
            }
            0xff42 => self.registers.scy,
            0xff43 => self.registers.scx,
            0xff44 => self.readable_ly(),
            0xff45 => self.registers.lyc,
            0xff46 => self.registers.dma,
            0xff47 => self.registers.bgp,
            0xff48 => self.registers.obp0,
            0xff49 => self.registers.obp1,
            0xff4a => self.registers.wy,
            0xff4b => self.registers.wx,
            _ => 0xff,
        }
    }

    pub(crate) fn write(&mut self, address: u16, value: u8) -> TickEffects {
        match address {
            0x8000..=0x9fff if self.vram_cpu_write_accessible() => {
                self.vram[usize::from(address - 0x8000)] = value;
                TickEffects::default()
            }
            0xfe00..=0xfe9f if self.oam_cpu_write_accessible() => {
                self.oam[usize::from(address - 0xfe00)] = value;
                TickEffects::default()
            }
            0xff40 => self.write_lcdc(value),
            0xff41 => {
                let transient = self.evaluate_stat_with_enables(0x78);
                self.registers.stat.write(value);
                transient.union(self.evaluate_stat())
            }
            0xff42 => {
                self.registers.scy = value;
                TickEffects::default()
            }
            0xff43 => {
                self.registers.scx = value;
                TickEffects::default()
            }
            0xff45 => {
                self.registers.lyc = value;
                if self.registers.lcdc.enabled() {
                    self.update_lyc_comparison();
                }
                self.evaluate_stat()
            }
            0xff46 => {
                self.registers.dma = value;
                TickEffects::default()
            }
            0xff47 => {
                self.registers.bgp = value;
                TickEffects::default()
            }
            0xff48 => {
                self.registers.obp0 = value;
                TickEffects::default()
            }
            0xff49 => {
                self.registers.obp1 = value;
                TickEffects::default()
            }
            0xff4a => {
                self.registers.wy = value;
                TickEffects::default()
            }
            0xff4b => {
                self.registers.wx = value;
                TickEffects::default()
            }
            _ => TickEffects::default(),
        }
    }

    fn write_lcdc(&mut self, value: u8) -> TickEffects {
        let was_enabled = self.registers.lcdc.enabled();
        self.registers.lcdc.write(value);
        let enabled = self.registers.lcdc.enabled();
        if was_enabled && !enabled {
            self.mode = LcdMode::HBlank;
            self.dot = 0;
            self.internal_line = 0;
            self.window_line = 0;
            self.startup_line = false;
            self.lcd_phase = 0;
            self.selected_sprites.clear();
            self.visible_hblank_dot = MODE2_DOTS + BASE_MODE3_DOTS;
            return TickEffects::default();
        }
        if !was_enabled && enabled {
            self.mode = LcdMode::HBlank;
            self.dot = 8;
            self.internal_line = 0;
            self.window_line = 0;
            self.blank_first_frame = true;
            self.startup_line = true;
            self.lcd_phase = 2;
            self.update_lyc_comparison();
            self.selected_sprites.clear();
            self.mode3_end_dot = MODE2_DOTS + BASE_MODE3_DOTS;
            self.visible_hblank_dot = self.mode3_end_dot;
        }
        self.evaluate_stat()
    }

    pub(crate) fn dma_write_oam(&mut self, index: u8, value: u8) {
        if let Some(byte) = self.oam.get_mut(usize::from(index)) {
            *byte = value;
        }
    }

    pub(crate) fn tick(&mut self, t_cycles: u32) -> TickEffects {
        if !self.registers.lcdc.enabled() {
            return TickEffects::default();
        }
        let mut effects = TickEffects::default();
        for _ in 0..t_cycles {
            effects = effects.union(self.tick_one());
        }
        effects
    }

    pub(crate) const fn frame_ready(&self) -> bool {
        self.pending_frame.is_some()
    }

    pub(crate) fn take_frame(&mut self) -> Option<Frame> {
        self.pending_frame.take()
    }

    fn publish_frame(&mut self) {
        if self.blank_first_frame {
            self.framebuffer.fill(0xff);
            self.blank_first_frame = false;
        }
        self.pending_frame = Some(
            Frame::new(self.next_sequence, self.framebuffer.clone())
                .expect("PPU framebuffer always has fixed DMG dimensions"),
        );
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .expect("frame sequence cannot overflow during a process lifetime");
    }

    pub(crate) const fn readable_ly(&self) -> u8 {
        if self.internal_line == 153 && self.dot >= 4 {
            0
        } else if self.lcd_phase == 0 && self.internal_line < 153 && self.dot >= 452 {
            self.internal_line + 1
        } else {
            self.internal_line
        }
    }

    fn update_lyc_comparison(&mut self) {
        self.lyc_equal = self.readable_ly() == self.registers.lyc;
    }

    fn vram_cpu_accessible(&self) -> bool {
        if !self.registers.lcdc.enabled() || self.internal_line >= VISIBLE_LINES {
            return true;
        }
        if self.startup_line {
            self.dot < self.mode3_start_dot() || self.dot >= self.access_hblank_dot()
        } else {
            self.dot < MODE2_DOTS || self.dot >= self.access_hblank_dot()
        }
    }

    fn vram_cpu_write_accessible(&self) -> bool {
        self.vram_cpu_accessible()
            || (self.registers.lcdc.enabled()
                && !self.startup_line
                && self.internal_line < VISIBLE_LINES
                && self.dot == MODE2_DOTS)
    }

    fn oam_cpu_accessible(&self) -> bool {
        if !self.registers.lcdc.enabled() || self.internal_line >= VISIBLE_LINES {
            return true;
        }
        if self.startup_line {
            self.dot < self.mode3_start_dot() || self.dot >= self.access_hblank_dot()
        } else {
            self.dot >= self.access_hblank_dot()
        }
    }

    const fn access_hblank_dot(&self) -> u16 {
        if self.lcd_phase == 0 {
            self.visible_hblank_dot
        } else {
            self.mode3_end_dot
        }
    }

    fn oam_cpu_write_accessible(&self) -> bool {
        self.oam_cpu_accessible()
            || (self.registers.lcdc.enabled()
                && !self.startup_line
                && self.internal_line < VISIBLE_LINES
                && (self.dot < 4 || self.dot == MODE2_DOTS))
    }

    #[cfg(test)]
    pub(crate) const fn dot(&self) -> u16 {
        self.dot
    }

    #[cfg(test)]
    pub(crate) const fn mode(&self) -> LcdMode {
        self.mode
    }

    #[cfg(test)]
    pub(crate) const fn window_line(&self) -> u8 {
        self.window_line
    }

    #[cfg(test)]
    pub(crate) fn mode3_end_dot(&mut self) -> u16 {
        self.selected_sprites = self.select_sprites_for_line(self.internal_line);
        self.compute_mode3_end_dot()
    }

    #[cfg(test)]
    pub(crate) fn last_row_source(&self, x: usize) -> render::PixelSource {
        self.last_rendered_row
            .as_ref()
            .expect("a row was rendered")
            .source_at(x)
    }
}

impl VideoDevice for Ppu {
    fn read(&self, address: u16) -> u8 {
        Ppu::read(self, address)
    }

    fn write(&mut self, address: u16, value: u8) -> TickEffects {
        Ppu::write(self, address, value)
    }

    fn dma_write_oam(&mut self, index: u8, value: u8) {
        Ppu::dma_write_oam(self, index, value);
    }

    fn tick(&mut self, t_cycles: u32) -> TickEffects {
        Ppu::tick(self, t_cycles)
    }

    fn frame_ready(&self) -> bool {
        Ppu::frame_ready(self)
    }

    fn take_frame(&mut self) -> Option<Frame> {
        Ppu::take_frame(self)
    }
}
