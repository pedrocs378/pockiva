use crate::SCREEN_WIDTH;

use super::{Ppu, map_palette};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum PixelSource {
    #[default]
    Background,
    Window,
    Sprite(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedRow {
    pub(crate) rgba: [[u8; 4]; SCREEN_WIDTH],
    pub(crate) colors: [u8; SCREEN_WIDTH],
    pub(crate) sources: [PixelSource; SCREEN_WIDTH],
    pub(crate) winning_sprites: [Option<u8>; SCREEN_WIDTH],
    pub(crate) window_drawn: bool,
}

impl RenderedRow {
    fn white() -> Self {
        Self {
            rgba: [[0xff; 4]; SCREEN_WIDTH],
            colors: [0; SCREEN_WIDTH],
            sources: [PixelSource::Background; SCREEN_WIDTH],
            winning_sprites: [None; SCREEN_WIDTH],
            window_drawn: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn rgba_at(&self, x: usize) -> [u8; 4] {
        self.rgba[x]
    }

    #[cfg(test)]
    pub(crate) fn source_at(&self, x: usize) -> PixelSource {
        self.sources[x]
    }

    #[cfg(test)]
    pub(crate) fn winning_sprite_at(&self, x: usize) -> Option<u8> {
        self.winning_sprites[x]
    }
}

pub(crate) fn tile_row_address(unsigned: bool, tile: u8, row: u8) -> u16 {
    if unsigned {
        0x8000 + u16::from(tile) * 16 + u16::from(row) * 2
    } else {
        let signed_tile = i8::from_ne_bytes([tile]);
        u16::try_from(0x9000_i32 + i32::from(signed_tile) * 16 + i32::from(row) * 2)
            .expect("signed DMG tile data always remains in VRAM")
    }
}

pub(crate) const fn decode_tile_row(low: u8, high: u8) -> [u8; 8] {
    let mut pixels = [0; 8];
    let mut index = 0;
    while index < 8 {
        let bit = 7 - index;
        pixels[index] = ((high >> bit) & 1) << 1 | ((low >> bit) & 1);
        index += 1;
    }
    pixels
}

impl Ppu {
    fn tile_color(&self, map_base: u16, pixel_x: u8, pixel_y: u8) -> u8 {
        let map_offset = u16::from(pixel_y / 8) * 32 + u16::from(pixel_x / 8);
        let tile = self.vram[usize::from(map_base + map_offset - 0x8000)];
        let address = tile_row_address(self.registers.lcdc.tile_data_unsigned(), tile, pixel_y & 7);
        let offset = usize::from(address - 0x8000);
        decode_tile_row(self.vram[offset], self.vram[offset + 1])[usize::from(pixel_x & 7)]
    }

    pub(crate) fn background_color_numbers(&self, line: u8) -> [u8; SCREEN_WIDTH] {
        let mut colors = [0; SCREEN_WIDTH];
        if !self.registers.lcdc.background_enabled() {
            return colors;
        }
        let map_base = if self.registers.lcdc.background_map_high() {
            0x9c00
        } else {
            0x9800
        };
        let map_y = line.wrapping_add(self.registers.scy);
        for (x, color) in colors.iter_mut().enumerate() {
            let map_x = u8::try_from(x)
                .expect("DMG scanline x fits u8")
                .wrapping_add(self.registers.scx);
            *color = self.tile_color(map_base, map_x, map_y);
        }
        colors
    }

    pub(crate) fn window_visible_on_line(&self, line: u8) -> bool {
        self.registers.lcdc.background_enabled()
            && self.registers.lcdc.window_enabled()
            && line >= self.registers.wy
            && self.registers.wx <= 166
    }

    pub(crate) fn render_scanline(&self, line: u8) -> RenderedRow {
        let mut row = RenderedRow::white();
        row.colors = self.background_color_numbers(line);
        for x in 0..SCREEN_WIDTH {
            row.rgba[x] = map_palette(self.registers.bgp, row.colors[x]);
        }

        if self.window_visible_on_line(line) {
            let origin = i16::from(self.registers.wx) - 7;
            let map_base = if self.registers.lcdc.window_map_high() {
                0x9c00
            } else {
                0x9800
            };
            for screen_x in 0..SCREEN_WIDTH {
                let window_x = i16::try_from(screen_x).expect("DMG scanline x fits i16") - origin;
                if !(0..=255).contains(&window_x) {
                    continue;
                }
                let color = self.tile_color(
                    map_base,
                    u8::try_from(window_x).expect("checked window x fits u8"),
                    self.window_line,
                );
                row.colors[screen_x] = color;
                row.rgba[screen_x] = map_palette(self.registers.bgp, color);
                row.sources[screen_x] = PixelSource::Window;
                row.window_drawn = true;
            }
        }

        if self.registers.lcdc.objects_enabled() {
            let mut sprites = self.selected_sprites.clone();
            sprites.sort_by_key(|sprite| (sprite.raw_x, sprite.oam_index));
            for screen_x in 0..SCREEN_WIDTH {
                for sprite in &sprites {
                    let color = self.sprite_color_number(
                        *sprite,
                        i16::try_from(screen_x).expect("DMG scanline x fits i16"),
                        line,
                    );
                    if color == 0 {
                        continue;
                    }
                    if sprite.flags & 0x80 != 0 && row.colors[screen_x] != 0 {
                        break;
                    }
                    let palette = if sprite.flags & 0x10 != 0 {
                        self.registers.obp1
                    } else {
                        self.registers.obp0
                    };
                    row.rgba[screen_x] = map_palette(palette, color);
                    row.sources[screen_x] = PixelSource::Sprite(sprite.oam_index);
                    row.winning_sprites[screen_x] = Some(sprite.oam_index);
                    break;
                }
            }
        }
        row
    }

    pub(super) fn store_scanline(&mut self, line: u8) {
        let row = self.render_scanline(line);
        if !self.blank_first_frame {
            let start = usize::from(line) * SCREEN_WIDTH * 4;
            for (index, rgba) in row.rgba.iter().enumerate() {
                self.framebuffer[start + index * 4..start + index * 4 + 4].copy_from_slice(rgba);
            }
        }
        if row.window_drawn {
            self.window_line = self.window_line.wrapping_add(1);
        }
        self.last_rendered_row = Some(row);
    }
}
