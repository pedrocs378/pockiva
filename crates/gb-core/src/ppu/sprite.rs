use super::Ppu;
use super::render::decode_tile_row;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectedSprite {
    pub(crate) raw_y: u8,
    pub(crate) raw_x: u8,
    pub(crate) tile: u8,
    pub(crate) flags: u8,
    pub(crate) oam_index: u8,
}

impl Ppu {
    pub(crate) fn select_sprites_for_line(&self, line: u8) -> Vec<SelectedSprite> {
        let height = i16::from(self.registers.lcdc.object_height());
        let screen_line = i16::from(line);
        self.oam
            .as_slice()
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter_map(|(index, bytes)| {
                let top = i16::from(bytes[0]) - 16;
                (screen_line >= top && screen_line < top + height).then_some(SelectedSprite {
                    raw_y: bytes[0],
                    raw_x: bytes[1],
                    tile: bytes[2],
                    flags: bytes[3],
                    oam_index: u8::try_from(index).expect("DMG OAM index fits u8"),
                })
            })
            .take(10)
            .collect()
    }

    pub(crate) fn sprite_color_number(
        &self,
        sprite: SelectedSprite,
        screen_x: i16,
        line: u8,
    ) -> u8 {
        let left = i16::from(sprite.raw_x) - 8;
        let top = i16::from(sprite.raw_y) - 16;
        let height = i16::from(self.registers.lcdc.object_height());
        let mut x = screen_x - left;
        let mut y = i16::from(line) - top;
        if !(0..8).contains(&x) || !(0..height).contains(&y) {
            return 0;
        }
        if sprite.flags & 0x20 != 0 {
            x = 7 - x;
        }
        if sprite.flags & 0x40 != 0 {
            y = height - 1 - y;
        }

        let (tile, row) = if height == 16 {
            let base = sprite.tile & 0xfe;
            (
                base.wrapping_add(u8::try_from(y / 8).expect("sprite tile half fits u8")),
                u8::try_from(y & 7).expect("sprite row fits u8"),
            )
        } else {
            (
                sprite.tile,
                u8::try_from(y).expect("8x8 sprite row fits u8"),
            )
        };
        let address = 0x8000_u16 + u16::from(tile) * 16 + u16::from(row) * 2;
        let low = self.vram[usize::from(address - 0x8000)];
        let high = self.vram[usize::from(address - 0x8000 + 1)];
        decode_tile_row(low, high)[usize::try_from(x).expect("sprite x is non-negative")]
    }

    pub(super) fn sprite_mode3_penalty(&self) -> u16 {
        if !self.registers.lcdc.objects_enabled() {
            return 0;
        }

        let mut sprites = self.selected_sprites.clone();
        sprites.sort_by_key(|sprite| (sprite.raw_x, sprite.oam_index));
        let mut charged_tiles = [false; 64];
        let mut penalty = 0_u16;
        for sprite in sprites {
            if sprite.raw_x >= 168 {
                continue;
            }
            let screen_x = i16::from(sprite.raw_x) - 8;
            let window_origin = i16::from(self.registers.wx) - 7;
            let (tile, local_x) = if sprite.raw_x >= 8
                && self.window_visible_on_line(self.internal_line)
                && screen_x >= window_origin
            {
                let window_x =
                    u8::try_from(screen_x - window_origin).expect("visible window x fits u8");
                (32 + usize::from(window_x / 8), window_x & 7)
            } else {
                let map_x = sprite.raw_x.wrapping_add(self.registers.scx);
                (usize::from(map_x / 8), map_x & 7)
            };
            if !charged_tiles[tile] {
                let pixels_strictly_right = 7_u8.saturating_sub(local_x);
                penalty += u16::from(pixels_strictly_right.saturating_sub(2));
                charged_tiles[tile] = true;
            }
            penalty += 6;
        }
        penalty
    }
}
