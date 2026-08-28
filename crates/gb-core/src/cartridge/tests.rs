use super::Cartridge;
use super::mapper::Mapper;
use super::mbc1::Mbc1;
use super::mbc3::Mbc3;
use super::mbc5::Mbc5;
use crate::{BatteryState, CompatibilityMode, CoreError, MapperKind};

fn rom_size_code(banks: usize) -> u8 {
    match banks {
        2 => 0,
        4 => 1,
        8 => 2,
        16 => 3,
        32 => 4,
        64 => 5,
        128 => 6,
        256 => 7,
        512 => 8,
        72 => 0x52,
        80 => 0x53,
        96 => 0x54,
        _ => panic!("unsupported synthetic bank count"),
    }
}

fn numbered_rom(banks: usize, type_byte: u8, ram_code: u8) -> Vec<u8> {
    let mut rom = vec![0; banks * 0x4000];
    for bank in 0..banks {
        rom[bank * 0x4000..(bank + 1) * 0x4000].fill(bank as u8);
        rom[bank * 0x4000] = bank as u8;
        rom[bank * 0x4000 + 1] = (bank >> 8) as u8;
    }
    rom[0x0134..0x013b].copy_from_slice(b"PED-35 ");
    rom[0x0143] = 0;
    rom[0x0147] = type_byte;
    rom[0x0148] = rom_size_code(banks);
    rom[0x0149] = ram_code;
    fix_checksum(&mut rom);
    rom
}

fn fix_checksum(rom: &mut [u8]) {
    rom[0x014d] = rom[0x0134..=0x014c]
        .iter()
        .fold(0_u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
}

#[test]
fn header_validation_and_identity_are_deterministic() {
    let rom = numbered_rom(2, 0x00, 0);
    let cartridge = Cartridge::load(&rom, None, 0).expect("valid ROM loads");
    assert_eq!(cartridge.metadata().title, "PED-35");
    assert_eq!(cartridge.metadata().mapper, MapperKind::RomOnly);
    assert_eq!(cartridge.metadata().compatibility, CompatibilityMode::Dmg);
    assert_eq!(cartridge.metadata().rom_identity.len(), 64);

    let mut changed = rom.clone();
    changed[0x0200] ^= 1;
    assert_ne!(
        cartridge.metadata().rom_identity,
        Cartridge::load(&changed, None, 0)
            .expect("valid changed ROM loads")
            .metadata()
            .rom_identity
    );
}

#[test]
fn cgb_only_bad_checksum_and_unsupported_mapper_are_typed_errors() {
    let mut cgb = numbered_rom(2, 0x00, 0);
    cgb[0x0143] = 0xc0;
    fix_checksum(&mut cgb);
    assert!(matches!(
        Cartridge::load(&cgb, None, 0),
        Err(CoreError::UnsupportedCgbOnlyCartridge)
    ));

    let mut bad_checksum = numbered_rom(2, 0x00, 0);
    bad_checksum[0x014d] ^= 1;
    assert!(matches!(
        Cartridge::load(&bad_checksum, None, 0),
        Err(CoreError::InvalidRom(reason)) if reason.contains("checksum")
    ));

    let unsupported = numbered_rom(2, 0x06, 0);
    assert!(matches!(
        Cartridge::load(&unsupported, None, 0),
        Err(CoreError::UnsupportedMapper(0x06))
    ));
}

#[test]
fn every_supported_type_byte_constructs_its_declared_mapper() {
    for (type_byte, mapper, ram_code) in [
        (0x00, MapperKind::RomOnly, 0),
        (0x08, MapperKind::RomOnly, 2),
        (0x09, MapperKind::RomOnly, 2),
        (0x01, MapperKind::Mbc1, 0),
        (0x02, MapperKind::Mbc1, 2),
        (0x03, MapperKind::Mbc1, 3),
        (0x0f, MapperKind::Mbc3, 0),
        (0x10, MapperKind::Mbc3, 3),
        (0x11, MapperKind::Mbc3, 0),
        (0x12, MapperKind::Mbc3, 2),
        (0x13, MapperKind::Mbc3, 3),
        (0x19, MapperKind::Mbc5, 0),
        (0x1a, MapperKind::Mbc5, 2),
        (0x1b, MapperKind::Mbc5, 3),
        (0x1c, MapperKind::Mbc5, 0),
        (0x1d, MapperKind::Mbc5, 2),
        (0x1e, MapperKind::Mbc5, 3),
    ] {
        let cartridge = Cartridge::load(&numbered_rom(2, type_byte, ram_code), None, 0)
            .expect("supported mapper loads");
        assert_eq!(cartridge.metadata().mapper, mapper);
    }
}

#[test]
fn mbc1_aliases_forbidden_switchable_banks_and_gates_ram() {
    let mut mapper =
        Mbc1::new(numbered_rom(128, 0x03, 3), 0x8000, true, None).expect("MBC1 constructs");
    mapper.write_rom(0x2000, 0, 0);
    assert_eq!(mapper.read_rom(0x4000), 1);
    mapper.write_rom(0x4000, 1, 0);
    assert_eq!(mapper.read_rom(0x4000), 0x21);

    assert_eq!(mapper.read_ram(0xa000, 0), 0xff);
    mapper.write_rom(0x0000, 0x0a, 0);
    mapper.write_ram(0xa000, 0x5a, 0);
    mapper.write_rom(0x0000, 0, 0);
    assert_eq!(mapper.read_ram(0xa000, 0), 0xff);
    mapper.write_rom(0x0000, 0x0a, 0);
    assert_eq!(mapper.read_ram(0xa000, 0), 0x5a);
}

#[test]
fn mbc3_latch_freezes_snapshot_and_persists_exact_schema() {
    let mut mapper = Mbc3::new(numbered_rom(4, 0x10, 3), 0x8000, true, true, None, 10_000)
        .expect("MBC3 constructs");
    mapper.write_rom(0x0000, 0x0a, 10_000);
    mapper.write_rom(0x4000, 0x08, 10_000);
    mapper.write_rom(0x6000, 0, 10_000);
    mapper.write_rom(0x6000, 1, 10_000);
    assert_eq!(mapper.read_ram(0xa000, 10_030), 0);
    mapper.write_rom(0x6000, 0, 10_030);
    mapper.write_rom(0x6000, 1, 10_030);
    assert_eq!(mapper.read_ram(0xa000, 10_030), 30);

    let state = mapper.battery_state(10_030).expect("battery state exists");
    assert_eq!(state.mapper_data().len(), 22);
    assert_eq!(&state.mapper_data()[..4], b"M3R1");
    Mbc3::new(
        numbered_rom(4, 0x10, 3),
        0x8000,
        true,
        true,
        Some(&state),
        13_691,
    )
    .expect("persisted RTC restores");
}

#[test]
fn mbc5_uses_nine_rom_bits_and_masks_rumble_from_ram_bank() {
    let mut mapper =
        Mbc5::new(numbered_rom(512, 0x1e, 3), 0x8000, true, true, None).expect("MBC5 constructs");
    mapper.write_rom(0x2000, 0, 0);
    mapper.write_rom(0x3000, 1, 0);
    assert_eq!(mapper.read_rom(0x4000), 0);
    assert_eq!(mapper.read_rom(0x4001), 1);
    mapper.write_rom(0x4000, 0x0b, 0);
    assert_eq!(mapper.selected_ram_bank(), 3);
}

#[test]
fn persisted_state_requires_version_ram_size_and_mapper_schema() {
    let rom = numbered_rom(2, 0x09, 2);
    let wrong_version = BatteryState::new(2, vec![0; 0x2000], Vec::new());
    assert!(Cartridge::load(&rom, Some(&wrong_version), 0).is_err());
    let wrong_ram = BatteryState::new(1, vec![0; 1], Vec::new());
    assert!(Cartridge::load(&rom, Some(&wrong_ram), 0).is_err());
    let wrong_mapper = BatteryState::new(1, vec![0; 0x2000], vec![1]);
    assert!(Cartridge::load(&rom, Some(&wrong_mapper), 0).is_err());
}

fn assert_send<T: Send>() {}

#[test]
fn mapper_and_cartridge_can_cross_thread_boundary() {
    assert_send::<Box<dyn Mapper>>();
    assert_send::<Cartridge>();
}
