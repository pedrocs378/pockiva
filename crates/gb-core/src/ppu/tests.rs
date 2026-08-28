use super::render::{PixelSource, decode_tile_row, tile_row_address};
use super::sprite::SelectedSprite;
use super::*;
use crate::interrupts::Interrupt;

fn disabled_ppu() -> Ppu {
    let mut ppu = Ppu::post_boot_dmg();
    ppu.write(0xff40, 0x11);
    ppu
}

fn set_tile_row(ppu: &mut Ppu, tile: u8, row: u8, colors: [u8; 8]) {
    let mut low = 0;
    let mut high = 0;
    for (x, color) in colors.into_iter().enumerate() {
        let bit = 7 - x;
        low |= (color & 1) << bit;
        high |= ((color >> 1) & 1) << bit;
    }
    let address = 0x8000 + u16::from(tile) * 16 + u16::from(row) * 2;
    ppu.write(address, low);
    ppu.write(address + 1, high);
}

fn add_sprite(ppu: &mut Ppu, index: u8, raw_y: u8, raw_x: u8, tile: u8, flags: u8) {
    let base = 0xfe00 + u16::from(index) * 4;
    for (offset, value) in [raw_y, raw_x, tile, flags].into_iter().enumerate() {
        let offset = u16::try_from(offset).expect("sprite field offset fits in u16");
        ppu.write(base + offset, value);
    }
}

#[test]
fn dmg_palette_register_maps_color_numbers_to_neutral_rgba() {
    assert_eq!(map_palette(0b11_10_01_00, 0), [0xff, 0xff, 0xff, 0xff]);
    assert_eq!(map_palette(0b11_10_01_00, 1), [0xaa, 0xaa, 0xaa, 0xff]);
    assert_eq!(map_palette(0b11_10_01_00, 2), [0x55, 0x55, 0x55, 0xff]);
    assert_eq!(map_palette(0b11_10_01_00, 3), [0x00, 0x00, 0x00, 0xff]);
}

#[test]
fn post_boot_dmg_registers_start_lcd_in_mode_two() {
    let ppu = Ppu::post_boot_dmg();
    assert_eq!(ppu.read(0xff40), 0x91);
    assert_eq!(ppu.read(0xff41) & 0x03, 2);
    assert_eq!(ppu.read(0xff44), 0);
    assert_eq!(ppu.read(0xff47), 0xfc);
    assert_eq!(ppu.read(0xff48), 0xff);
    assert_eq!(ppu.read(0xff49), 0xff);
}

#[test]
fn stat_lcd_register_masks_and_lifecycle_are_deterministic() {
    let mut ppu = Ppu::post_boot_dmg();
    assert_eq!(ppu.write(0xff41, 0xff).requested_interrupts.bits(), 0x02);
    assert_eq!(ppu.read(0xff41), 0xfe);
    ppu.write(0xff44, 99);
    assert_eq!(ppu.read(0xff44), 0);
    ppu.tick(92);
    ppu.write(0xff40, 0x11);
    assert_eq!(
        (ppu.read(0xff44), ppu.dot(), ppu.mode()),
        (0, 0, LcdMode::HBlank)
    );
    assert_eq!(ppu.tick(912), TickEffects::default());
    ppu.write(0xff40, 0x91);
    assert_eq!(
        (ppu.read(0xff44), ppu.dot(), ppu.mode()),
        (0, 8, LcdMode::HBlank)
    );
}

#[test]
fn lcd_enable_starts_in_mode_zero_and_first_line_is_two_dots_late() {
    let mut ppu = disabled_ppu();
    ppu.write(0xff40, 0x91);
    assert_eq!(ppu.mode(), LcdMode::HBlank);
    assert_eq!(ppu.dot(), 8);
    ppu.tick(75);
    assert_eq!(ppu.mode(), LcdMode::HBlank);
    ppu.tick(1);
    assert_eq!(ppu.mode(), LcdMode::Drawing);
    ppu.tick(376);
    assert_eq!((ppu.read(0xff44), ppu.mode()), (1, LcdMode::OamScan));
}

#[test]
fn lcd_disabled_retains_coincidence_until_comparison_clock_restarts() {
    let mut ppu = Ppu::post_boot_dmg();
    ppu.write(0xff41, 0x40);
    ppu.write(0xff45, 0);
    ppu.write(0xff40, 0x11);
    ppu.write(0xff45, 1);
    assert_eq!(ppu.read(0xff41) & 0x04, 0x04);
    assert_eq!(ppu.write(0xff40, 0x91).requested_interrupts.bits(), 0);
    assert_eq!(ppu.read(0xff41) & 0x04, 0);

    ppu.write(0xff40, 0x11);
    ppu.write(0xff45, 0);
    assert_eq!(ppu.read(0xff41) & 0x04, 0);
    assert_eq!(ppu.write(0xff40, 0x91).requested_interrupts.bits(), 0x02);
    assert_eq!(ppu.read(0xff41) & 0x04, 0x04);
}

#[test]
fn dmg_mode_two_stat_source_pulses_when_vblank_starts() {
    let mut ppu = Ppu::post_boot_dmg();
    ppu.write(0xff41, 0x20);
    ppu.tick(456 * 143);
    let effects = ppu.tick(456);
    assert_ne!(
        effects.requested_interrupts.bits() & Interrupt::LcdStat.bit(),
        0
    );
    assert_eq!(ppu.mode(), LcdMode::VBlank);
    ppu.tick(1);
    assert!(!ppu.stat_line_high);
}

#[test]
fn timing_uses_80_172_204_dots_and_wraps_after_line_153() {
    let mut ppu = Ppu::post_boot_dmg();
    ppu.tick(79);
    assert_eq!(ppu.mode(), LcdMode::OamScan);
    ppu.tick(1);
    assert_eq!(ppu.mode(), LcdMode::Drawing);
    ppu.tick(171);
    assert_eq!(ppu.mode(), LcdMode::Drawing);
    ppu.tick(1);
    assert_eq!(ppu.mode(), LcdMode::HBlank);
    ppu.tick(204);
    assert_eq!((ppu.read(0xff44), ppu.mode()), (1, LcdMode::OamScan));

    let mut ppu = Ppu::post_boot_dmg();
    let effects = ppu.tick(456 * 144);
    assert_ne!(
        effects.requested_interrupts.bits() & Interrupt::VBlank.bit(),
        0
    );
    assert_eq!((ppu.read(0xff44), ppu.mode()), (144, LcdMode::VBlank));
    ppu.tick(456 * 9 + 4);
    assert_eq!(ppu.read(0xff44), 0);
    ppu.tick(452);
    assert_eq!((ppu.read(0xff44), ppu.mode()), (0, LcdMode::OamScan));
}

#[test]
fn stat_requests_only_on_combined_line_rising_edges() {
    let mut ppu = Ppu::post_boot_dmg();
    assert_eq!(ppu.write(0xff41, 0x20).requested_interrupts.bits(), 0x02);
    assert_eq!(ppu.tick(4).requested_interrupts.bits(), 0);
    ppu.tick(452);
    assert_eq!(ppu.tick(456).requested_interrupts.bits() & 0x02, 0x02);

    let mut ppu = Ppu::post_boot_dmg();
    ppu.write(0xff41, 0x40);
    ppu.write(0xff45, 1);
    assert_eq!(ppu.tick(456).requested_interrupts.bits() & 0x02, 0x02);
    ppu.tick(456 * 152 + 4);
    ppu.write(0xff45, 0);
    assert_eq!(ppu.read(0xff41) & 0x04, 0x04);
}

#[test]
fn access_is_blocked_by_drawing_and_oam_scan_but_dma_is_not() {
    let mut ppu = disabled_ppu();
    ppu.write(0x8000, 0x12);
    ppu.write(0xfe00, 0x34);
    ppu.write(0xff40, 0x91);
    assert_eq!(ppu.read(0xfe00), 0x34);
    ppu.dma_write_oam(0, 0x77);
    ppu.tick(76);
    assert_eq!(ppu.read(0x8000), 0xff);
    ppu.write(0x8000, 0x99);
    ppu.tick(172);
    assert_eq!(ppu.read(0x8000), 0x12);
    assert_eq!(ppu.read(0xfe00), 0x77);
}

#[test]
fn tile_data_select_and_planar_decode_cover_signed_indices() {
    assert_eq!(tile_row_address(true, 0x00, 3), 0x8006);
    assert_eq!(tile_row_address(true, 0xff, 7), 0x8ffe);
    assert_eq!(tile_row_address(false, 0x00, 0), 0x9000);
    assert_eq!(tile_row_address(false, 0x80, 0), 0x8800);
    assert_eq!(tile_row_address(false, 0x7f, 7), 0x97fe);
    assert_eq!(
        decode_tile_row(0b1000_0001, 0b0100_0001),
        [1, 2, 0, 0, 0, 0, 0, 3]
    );
}

#[test]
fn background_scroll_wraps_across_the_256_pixel_map() {
    let mut ppu = disabled_ppu();
    set_tile_row(&mut ppu, 1, 7, [1, 2, 0, 0, 0, 0, 0, 3]);
    ppu.write(0x9bff, 1);
    ppu.write(0x9be0, 1);
    ppu.write(0xff42, 255);
    ppu.write(0xff43, 255);
    ppu.write(0xff40, 0x91);
    let colors = ppu.background_color_numbers(0);
    assert_eq!(&colors[0..3], &[3, 1, 2]);
}

#[test]
fn window_origin_counter_and_penalty_follow_dmg_rules() {
    let mut ppu = disabled_ppu();
    set_tile_row(&mut ppu, 1, 0, [1; 8]);
    ppu.write(0x9800, 1);
    ppu.write(0xff4a, 2);
    ppu.write(0xff4b, 10);
    ppu.write(0xff40, 0xb1);
    ppu.tick(456 * 2 + 258);
    assert_eq!(ppu.window_line(), 1);
    assert_eq!(ppu.last_row_source(2), PixelSource::Background);
    assert_eq!(ppu.last_row_source(3), PixelSource::Window);

    let mut ppu = disabled_ppu();
    ppu.write(0xff40, 0xb1);
    ppu.write(0xff4a, 0);
    ppu.write(0xff4b, 7);
    assert_eq!(ppu.mode3_end_dot(), 262);
    ppu.write(0xff4b, 167);
    assert_eq!(ppu.mode3_end_dot(), 256);
}

#[test]
fn sprite_decode_keeps_first_ten_and_handles_eight_by_sixteen_flips() {
    let mut ppu = disabled_ppu();
    for index in 0..12 {
        add_sprite(&mut ppu, index, 16, 8 + index, 0, 0);
    }
    assert_eq!(
        ppu.select_sprites_for_line(0)
            .iter()
            .map(|sprite| sprite.oam_index)
            .collect::<Vec<_>>(),
        (0..10).collect::<Vec<_>>()
    );

    set_tile_row(&mut ppu, 3, 7, [0, 0, 0, 0, 0, 0, 0, 3]);
    let sprite = SelectedSprite {
        raw_y: 16,
        raw_x: 8,
        tile: 3,
        flags: 0x60,
        oam_index: 0,
    };
    ppu.write(0xff40, 0x97);
    assert_eq!(ppu.sprite_color_number(sprite, 0, 0), 3);
}

#[test]
fn sprite_priority_is_lower_x_then_oam_and_respects_background_priority() {
    let mut ppu = disabled_ppu();
    set_tile_row(&mut ppu, 1, 0, [1; 8]);
    set_tile_row(&mut ppu, 2, 0, [2; 8]);
    add_sprite(&mut ppu, 3, 16, 28, 1, 0);
    add_sprite(&mut ppu, 1, 16, 30, 2, 0);
    ppu.write(0xff40, 0x93);
    ppu.selected_sprites = ppu.select_sprites_for_line(0);
    let row = ppu.render_scanline(0);
    assert_eq!(row.winning_sprite_at(20), Some(3));
    assert_eq!(row.rgba_at(20), map_palette(ppu.read(0xff48), 1));

    let mut ppu = disabled_ppu();
    set_tile_row(&mut ppu, 0, 0, [0, 1, 0, 0, 0, 0, 0, 0]);
    set_tile_row(&mut ppu, 1, 0, [2; 8]);
    add_sprite(&mut ppu, 0, 16, 8, 1, 0x80);
    ppu.write(0xff40, 0x93);
    ppu.selected_sprites = ppu.select_sprites_for_line(0);
    let row = ppu.render_scanline(0);
    assert_eq!(row.source_at(0), PixelSource::Sprite(0));
    assert_eq!(row.source_at(1), PixelSource::Background);
}

#[test]
fn frame_is_published_once_at_vblank_and_unread_frames_are_replaced() {
    let mut ppu = Ppu::post_boot_dmg();
    ppu.tick(456 * 144 - 1);
    assert!(!ppu.frame_ready());
    ppu.tick(1);
    let frame = ppu.take_frame().expect("completed frame");
    assert_eq!((frame.sequence(), frame.rgba().len()), (1, FRAME_RGBA_LEN));
    assert!(!ppu.frame_ready());

    ppu.tick(456 * 154 * 3);
    let frame = ppu.take_frame().expect("latest frame");
    assert_eq!(frame.sequence(), 4);
    assert!(ppu.take_frame().is_none());
}

fn synthetic_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0; 0x8000];
    rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
    rom[0x0147] = 0;
    rom[0x0148] = 0;
    rom[0x0149] = 0;
    rom[0x014d] = rom[0x0134..=0x014c]
        .iter()
        .fold(0_u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
    rom
}

#[test]
fn rom_harness_names_asset_and_exact_bound_on_failure() {
    let bytes = synthetic_rom(&[0x00, 0x18, 0xfd]);
    let checksum_error = super::rom_tests::verify_bytes("synthetic.gb", &bytes, "wrong", 12)
        .expect_err("wrong checksum fails");
    assert!(checksum_error.contains("synthetic.gb"));
    assert!(checksum_error.contains("12 T-cycle bound"));

    let run_error = super::rom_tests::run_mooneye_bytes("synthetic.gb", &bytes, 12)
        .expect_err("missing signature fails");
    assert_eq!(
        run_error,
        "synthetic.gb timed out at the exact 12 T-cycle bound"
    );
}
