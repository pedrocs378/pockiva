use std::fmt::Write;

use sha2::{Digest, Sha256};

use crate::{CartridgeMetadata, CompatibilityMode, CoreError, MapperKind};

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub(super) struct Features {
    pub(super) mapper: MapperKind,
    pub(super) has_ram: bool,
    pub(super) has_battery: bool,
    pub(super) has_timer: bool,
    pub(super) has_rumble: bool,
}

pub(super) struct Header {
    pub(super) metadata: CartridgeMetadata,
    pub(super) features: Features,
    pub(super) rom_banks: usize,
    pub(super) ram_bytes: usize,
}

pub(super) fn parse(rom: &[u8]) -> Result<Header, CoreError> {
    if rom.len() < 0x150 {
        return Err(CoreError::InvalidRom(
            "image is shorter than its header".into(),
        ));
    }
    let compatibility = match rom[0x0143] {
        0xc0 => return Err(CoreError::UnsupportedCgbOnlyCartridge),
        0x80 => CompatibilityMode::DmgCompatible,
        _ => CompatibilityMode::Dmg,
    };
    let features = features(rom[0x0147])?;
    let rom_banks = rom_banks(rom[0x0148])?;
    let ram_bytes = ram_bytes(rom[0x0149])?;
    if rom.len() != rom_banks * 0x4000 {
        return Err(CoreError::InvalidRom(
            "image length does not match declared ROM size".into(),
        ));
    }
    let checksum = rom[0x0134..=0x014c]
        .iter()
        .fold(0_u8, |sum, byte| sum.wrapping_sub(*byte).wrapping_sub(1));
    if checksum != rom[0x014d] {
        return Err(CoreError::InvalidRom("header checksum mismatch".into()));
    }
    if ram_bytes != 0 && !features.has_ram {
        return Err(CoreError::InvalidRom(
            "header declares RAM for a cartridge type without RAM".into(),
        ));
    }
    validate_capacity(features, rom_banks, ram_bytes)?;

    let title_end = if matches!(rom[0x0143], 0x80 | 0xc0) {
        0x0143
    } else {
        0x0144
    };
    let title_bytes = &rom[0x0134..title_end];
    let title_len = title_bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(title_bytes.len());
    let title = String::from_utf8_lossy(&title_bytes[..title_len])
        .trim_end()
        .to_string();
    let digest = Sha256::digest(rom);
    let mut rom_identity = String::with_capacity(64);
    for byte in digest {
        write!(&mut rom_identity, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(Header {
        metadata: CartridgeMetadata {
            title,
            rom_identity,
            mapper: features.mapper,
            compatibility,
            ram_size_bytes: ram_bytes,
            has_battery: features.has_battery,
        },
        features,
        rom_banks,
        ram_bytes,
    })
}

fn features(type_byte: u8) -> Result<Features, CoreError> {
    let (mapper, has_ram, has_battery, has_timer, has_rumble) = match type_byte {
        0x00 => (MapperKind::RomOnly, false, false, false, false),
        0x08 => (MapperKind::RomOnly, true, false, false, false),
        0x09 => (MapperKind::RomOnly, true, true, false, false),
        0x01 => (MapperKind::Mbc1, false, false, false, false),
        0x02 => (MapperKind::Mbc1, true, false, false, false),
        0x03 => (MapperKind::Mbc1, true, true, false, false),
        0x0f => (MapperKind::Mbc3, false, true, true, false),
        0x10 => (MapperKind::Mbc3, true, true, true, false),
        0x11 => (MapperKind::Mbc3, false, false, false, false),
        0x12 => (MapperKind::Mbc3, true, false, false, false),
        0x13 => (MapperKind::Mbc3, true, true, false, false),
        0x19 => (MapperKind::Mbc5, false, false, false, false),
        0x1a => (MapperKind::Mbc5, true, false, false, false),
        0x1b => (MapperKind::Mbc5, true, true, false, false),
        0x1c => (MapperKind::Mbc5, false, false, false, true),
        0x1d => (MapperKind::Mbc5, true, false, false, true),
        0x1e => (MapperKind::Mbc5, true, true, false, true),
        other => return Err(CoreError::UnsupportedMapper(other)),
    };
    Ok(Features {
        mapper,
        has_ram,
        has_battery,
        has_timer,
        has_rumble,
    })
}

fn rom_banks(code: u8) -> Result<usize, CoreError> {
    match code {
        0x00..=0x08 => Ok(2_usize << code),
        0x52 => Ok(72),
        0x53 => Ok(80),
        0x54 => Ok(96),
        _ => Err(CoreError::InvalidRom("unsupported ROM size code".into())),
    }
}

fn ram_bytes(code: u8) -> Result<usize, CoreError> {
    match code {
        0x00 => Ok(0),
        0x01 => Ok(0x0800),
        0x02 => Ok(0x2000),
        0x03 => Ok(0x8000),
        0x04 => Ok(0x20_000),
        0x05 => Ok(0x10_000),
        _ => Err(CoreError::InvalidRom("unsupported RAM size code".into())),
    }
}

fn validate_capacity(
    features: Features,
    rom_banks: usize,
    ram_bytes: usize,
) -> Result<(), CoreError> {
    let external_ram_banks = ram_bytes.div_ceil(0x2000);
    let valid = match features.mapper {
        MapperKind::RomOnly => rom_banks <= 2 && ram_bytes <= 0x2000,
        MapperKind::Mbc1 | MapperKind::Mbc3 => rom_banks <= 128 && external_ram_banks <= 4,
        MapperKind::Mbc5 if features.has_rumble => rom_banks <= 512 && external_ram_banks <= 8,
        MapperKind::Mbc5 => rom_banks <= 512 && external_ram_banks <= 16,
    };
    if valid {
        Ok(())
    } else {
        Err(CoreError::InvalidRom(
            "declared cartridge capacity exceeds mapper limits".into(),
        ))
    }
}
