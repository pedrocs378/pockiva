use std::num::NonZeroU32;

use super::MachineBus;
use crate::cartridge::Cartridge;
use crate::cpu::CpuBus;

fn test_rom() -> Vec<u8> {
    let mut rom = vec![0; 0x8000];
    rom[0x0147] = 0;
    rom[0x0148] = 0;
    rom[0x0149] = 0;
    rom[0x014d] = rom[0x0134..=0x014c]
        .iter()
        .fold(0_u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
    rom
}

fn test_bus() -> MachineBus {
    MachineBus::new(
        Cartridge::load(&test_rom(), None, 0).expect("test ROM loads"),
        NonZeroU32::new(48_000).expect("non-zero"),
        0,
    )
}

#[test]
fn real_ppu_starts_in_post_boot_mode_and_advances_one_scanline() {
    let mut bus = test_bus();
    assert_eq!(bus.read_unclocked(0xff40), 0x91);
    assert_eq!(bus.read_unclocked(0xff44), 0);
    assert_eq!(bus.read_unclocked(0xff41) & 0x03, 2);

    for _ in 0..114 {
        bus.idle_m_cycle();
    }

    assert_eq!(bus.read_unclocked(0xff44), 1);
    assert_eq!(bus.elapsed_t_cycles(), 456);
}

#[test]
fn real_ppu_requests_vblank_and_publishes_one_fixed_rgba_frame() {
    let mut bus = test_bus();
    for _ in 0..(456 * 144 / 4) {
        bus.idle_m_cycle();
    }

    assert_ne!(bus.interrupts.read_if() & 0x01, 0);
    assert!(bus.frame_ready());
    let frame = bus.take_frame().expect("VBlank publishes a frame");
    assert_eq!(frame.rgba().len(), 160 * 144 * 4);
    assert!(!bus.frame_ready());
    assert!(bus.take_frame().is_none());
}

#[test]
fn stat_register_writes_request_interrupts_without_waiting_for_another_tick() {
    let mut bus = test_bus();
    bus.write_unclocked(0xff0f, 0);

    bus.write_unclocked(0xff41, 0x40);
    assert_ne!(bus.interrupts.read_if() & 0x02, 0);

    bus.write_unclocked(0xff45, 1);
    bus.write_unclocked(0xff0f, 0);
    bus.write_unclocked(0xff45, 0);
    assert_ne!(bus.interrupts.read_if() & 0x02, 0);
}

#[test]
fn wram_echoes_and_each_cpu_access_ticks_four_t_cycles() {
    let mut bus = test_bus();
    bus.write8(0xc123, 0xa5);
    assert_eq!(bus.read8(0xe123), 0xa5);
    assert_eq!(bus.elapsed_t_cycles(), 8);
}

#[test]
fn memory_map_boundaries_have_dmg_open_bus_behavior() {
    let mut bus = test_bus();
    bus.write8(0xff80, 0x11);
    bus.write8(0xfffe, 0x22);
    assert_eq!(bus.read8(0xff80), 0x11);
    assert_eq!(bus.read8(0xfffe), 0x22);
    assert_eq!(bus.read8(0xfea0), 0xff);
    assert_eq!(bus.read8(0xfeff), 0xff);
    assert_eq!(bus.read8(0xff7f), 0xff);
}

#[test]
fn oam_dma_copies_one_byte_per_machine_cycle_and_blocks_non_hram_cpu_access() {
    let mut bus = test_bus();
    for value in 0_u8..160 {
        bus.write_unclocked(0xc000 + u16::from(value), value);
    }
    bus.write8(0xff46, 0xc0);
    assert_eq!(bus.read8(0xc000), 0xff);
    bus.write8(0xff80, 0x55);
    assert_eq!(bus.read8(0xff80), 0x55);
    while bus.dma.active() {
        bus.idle_m_cycle();
    }
    assert_eq!(bus.elapsed_t_cycles(), 4 + 640);
    assert_eq!(bus.video.read(0xfe00), 0xff);
    let mut wait_cycles = 0;
    while bus.video.read(0xff41) & 0x03 != 0 {
        bus.idle_m_cycle();
        wait_cycles += 1;
        assert!(wait_cycles <= 114, "PPU reaches HBlank within one scanline");
    }
    for expected in 0_u8..160 {
        assert_eq!(bus.video.read(0xfe00 + u16::from(expected)), expected);
    }
}

#[test]
fn serial_capture_is_bounded() {
    let mut bus = test_bus();
    for value in 0..4200_u16 {
        bus.write_unclocked(0xff01, value.to_le_bytes()[0]);
        bus.write_unclocked(0xff02, 0x81);
    }
    assert_eq!(bus.serial_output().len(), 4096);
}

#[test]
fn timer_write_takes_effect_before_the_access_cycle_ticks() {
    let mut bus = test_bus();
    bus.write_unclocked(0xff04, 0);
    bus.write_unclocked(0xff05, 0);
    bus.write_unclocked(0xff07, 0b101);

    bus.write8(0xff04, 0);
    bus.idle_m_cycle();
    bus.idle_m_cycle();
    bus.idle_m_cycle();

    assert_eq!(bus.read_unclocked(0xff05), 1);
}

fn assert_send<T: Send>() {}

#[test]
fn complete_machine_bus_can_move_between_threads() {
    assert_send::<MachineBus>();
}
