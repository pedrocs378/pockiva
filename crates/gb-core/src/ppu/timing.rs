use crate::bus::devices::TickEffects;
use crate::interrupts::{Interrupt, InterruptMask};

use super::{
    BASE_MODE3_DOTS, DOTS_PER_LINE, LINES_PER_FRAME, LcdMode, MODE2_DOTS, Ppu, STARTUP_MODE3_DOT,
    VISIBLE_LINES,
};

impl Ppu {
    pub(super) fn stat_level_with_enables(&self, enables: u8) -> bool {
        let lcd_enabled = self.registers.lcdc.enabled();
        (lcd_enabled
            && self.mode == LcdMode::HBlank
            && (self.startup_line || self.dot >= self.mode3_end_dot)
            && enables & 0x08 != 0)
            || (lcd_enabled && self.mode == LcdMode::VBlank && enables & 0x10 != 0)
            || (lcd_enabled
                && ((self.mode == LcdMode::OamScan)
                    || (self.internal_line == VISIBLE_LINES && self.dot == 0))
                && enables & 0x20 != 0)
            || (self.lyc_equal && enables & 0x40 != 0)
    }

    pub(super) fn evaluate_stat(&mut self) -> TickEffects {
        self.evaluate_stat_with_enables(self.registers.stat.enables())
    }

    pub(super) fn evaluate_stat_with_enables(&mut self, enables: u8) -> TickEffects {
        let level = self.stat_level_with_enables(enables);
        let rising = level && !self.stat_line_high;
        self.stat_line_high = level;
        TickEffects {
            requested_interrupts: InterruptMask::from_bits(if rising {
                Interrupt::LcdStat.bit()
            } else {
                0
            }),
        }
    }

    pub(super) fn compute_mode3_end_dot(&self) -> u16 {
        let mut length = BASE_MODE3_DOTS + u16::from(self.registers.scx & 7);
        if self.window_visible_on_line(self.internal_line) {
            length += 6;
            if self.registers.wx == 0 && self.registers.scx & 7 != 0 {
                length -= 1;
            }
        }
        length = length.saturating_add(self.sprite_mode3_penalty());
        self.mode3_start_dot() + length.min(289)
    }

    pub(super) const fn mode3_start_dot(&self) -> u16 {
        if self.startup_line {
            STARTUP_MODE3_DOT
        } else {
            MODE2_DOTS + self.lcd_phase
        }
    }

    pub(super) fn tick_one(&mut self) -> TickEffects {
        let mut effects = TickEffects::default();
        self.dot += 1;

        if self.internal_line == 153 && self.dot == 4 {
            self.update_lyc_comparison();
            effects = effects.union(self.evaluate_stat());
        }

        if self.internal_line == VISIBLE_LINES && self.dot == 1 {
            effects = effects.union(self.evaluate_stat());
        }

        if !self.startup_line
            && self.internal_line < VISIBLE_LINES
            && self.lcd_phase != 0
            && self.dot == self.lcd_phase
        {
            self.mode = LcdMode::OamScan;
            self.update_lyc_comparison();
            effects = effects.union(self.evaluate_stat());
        }

        if self.internal_line < VISIBLE_LINES {
            if self.dot == self.mode3_start_dot() {
                self.mode = LcdMode::Drawing;
                self.selected_sprites = self.select_sprites_for_line(self.internal_line);
                self.mode3_end_dot = self.compute_mode3_end_dot();
                self.visible_hblank_dot = if self.sprite_mode3_penalty() == 0 {
                    self.mode3_end_dot
                } else {
                    self.mode3_end_dot - 1
                };
                effects = effects.union(self.evaluate_stat());
            } else if self.dot == self.visible_hblank_dot {
                self.store_scanline(self.internal_line);
                self.mode = LcdMode::HBlank;
                effects = effects.union(self.evaluate_stat());
            } else if self.dot == self.mode3_end_dot {
                effects = effects.union(self.evaluate_stat());
            }
        }

        if self.dot == DOTS_PER_LINE {
            self.dot = 0;
            self.internal_line += 1;
            self.startup_line = false;
            if self.internal_line == VISIBLE_LINES {
                self.mode = LcdMode::VBlank;
                self.publish_frame();
                effects.requested_interrupts = effects
                    .requested_interrupts
                    .union(InterruptMask::from_bits(Interrupt::VBlank.bit()));
            } else if self.internal_line == LINES_PER_FRAME {
                self.internal_line = 0;
                self.window_line = 0;
                self.mode = if self.lcd_phase == 0 {
                    LcdMode::OamScan
                } else {
                    LcdMode::HBlank
                };
                self.selected_sprites = self.select_sprites_for_line(0);
                self.mode3_end_dot = MODE2_DOTS + BASE_MODE3_DOTS;
                self.visible_hblank_dot = self.mode3_end_dot;
            } else if self.internal_line < VISIBLE_LINES {
                self.mode = if self.lcd_phase == 0 {
                    LcdMode::OamScan
                } else {
                    LcdMode::HBlank
                };
                self.selected_sprites = self.select_sprites_for_line(self.internal_line);
                self.mode3_end_dot = MODE2_DOTS + BASE_MODE3_DOTS;
                self.visible_hblank_dot = self.mode3_end_dot;
            }
            if self.lcd_phase == 0 || self.internal_line >= VISIBLE_LINES {
                self.update_lyc_comparison();
            } else if self.readable_ly() != self.registers.lyc {
                self.lyc_equal = false;
            }
            effects = effects.union(self.evaluate_stat());
        }
        effects
    }
}
