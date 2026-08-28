mod header;
mod mapper;
mod mbc1;
mod mbc3;
mod mbc5;
mod rom_only;

use crate::{BatteryState, CartridgeMetadata, CoreError, MapperKind};
use mapper::Mapper;

pub(crate) struct Cartridge {
    metadata: CartridgeMetadata,
    mapper: Box<dyn Mapper>,
}

impl Cartridge {
    pub(crate) fn load(
        rom: &[u8],
        persisted: Option<&BatteryState>,
        now: u64,
    ) -> Result<Self, CoreError> {
        let header = header::parse(rom)?;
        let features = header.features;
        debug_assert_eq!(header.rom_banks, rom.len() / 0x4000);
        let mapper: Box<dyn Mapper> = match features.mapper {
            MapperKind::RomOnly => Box::new(rom_only::RomOnly::new(
                rom.to_vec(),
                header.ram_bytes,
                features.has_battery,
                persisted,
            )?),
            MapperKind::Mbc1 => Box::new(mbc1::Mbc1::new(
                rom.to_vec(),
                header.ram_bytes,
                features.has_battery,
                persisted,
            )?),
            MapperKind::Mbc3 => Box::new(mbc3::Mbc3::new(
                rom.to_vec(),
                header.ram_bytes,
                features.has_battery,
                features.has_timer,
                persisted,
                now,
            )?),
            MapperKind::Mbc5 => Box::new(mbc5::Mbc5::new(
                rom.to_vec(),
                header.ram_bytes,
                features.has_rumble,
                features.has_battery,
                persisted,
            )?),
        };
        Ok(Self {
            metadata: header.metadata,
            mapper,
        })
    }

    pub(crate) const fn metadata(&self) -> &CartridgeMetadata {
        &self.metadata
    }
    pub(crate) fn read(&self, address: u16, now: u64) -> u8 {
        match address {
            0x0000..=0x7fff => self.mapper.read_rom(address),
            0xa000..=0xbfff => self.mapper.read_ram(address, now),
            _ => 0xff,
        }
    }
    pub(crate) fn write(&mut self, address: u16, value: u8, now: u64) {
        match address {
            0x0000..=0x7fff => self.mapper.write_rom(address, value, now),
            0xa000..=0xbfff => self.mapper.write_ram(address, value, now),
            _ => {}
        }
    }
    pub(crate) fn reset(&mut self, now: u64) {
        self.mapper.reset(now);
    }
    pub(crate) fn battery_state(&self, now: u64) -> Option<BatteryState> {
        self.mapper.battery_state(now)
    }
}

#[cfg(test)]
mod tests;
