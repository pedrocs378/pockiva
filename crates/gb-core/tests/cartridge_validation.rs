use std::num::NonZeroU32;

use gb_core::{Clock, CompatibilityMode, CoreError, EmulatorCore, GameBoy, MapperKind};

struct FixedClock;

impl Clock for FixedClock {
    fn unix_seconds(&self) -> u64 {
        123
    }
}

fn rom(type_byte: u8, cgb: u8) -> Vec<u8> {
    let mut rom = vec![0; 0x8000];
    rom[0x0134..0x013c].copy_from_slice(b"CORE ROM");
    rom[0x0143] = cgb;
    rom[0x0147] = type_byte;
    rom[0x0148] = 0;
    rom[0x0149] = 0;
    rom[0x014d] = rom[0x0134..=0x014c]
        .iter()
        .fold(0_u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
    rom
}

fn core() -> GameBoy<FixedClock> {
    GameBoy::new(FixedClock, NonZeroU32::new(48_000).expect("non-zero"))
}

#[test]
fn public_loader_reports_validated_metadata() {
    let mut core = core();
    let metadata = core.load_rom(&rom(0, 0x80), None).expect("ROM loads");
    assert_eq!(metadata.title, "CORE ROM");
    assert_eq!(metadata.mapper, MapperKind::RomOnly);
    assert_eq!(metadata.compatibility, CompatibilityMode::DmgCompatible);
    assert_eq!(metadata.rom_identity.len(), 64);
}

#[test]
fn public_loader_rejects_cgb_only_and_unknown_mapper() {
    let mut core = core();
    assert_eq!(
        core.load_rom(&rom(0, 0xc0), None),
        Err(CoreError::UnsupportedCgbOnlyCartridge)
    );
    assert_eq!(
        core.load_rom(&rom(0x06, 0), None),
        Err(CoreError::UnsupportedMapper(0x06))
    );
}
