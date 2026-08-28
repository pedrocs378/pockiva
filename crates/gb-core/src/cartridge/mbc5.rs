use crate::{BatteryState, CoreError};

use super::mapper::{Mapper, persisted_ram, ram_byte, rom_byte, write_ram_byte};

pub(super) struct Mbc5 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_banks: usize,
    ram_banks: usize,
    ram_enabled: bool,
    rom_bank: u16,
    ram_bank: u8,
    has_rumble: bool,
    has_battery: bool,
}

impl Mbc5 {
    pub(super) fn new(
        rom: Vec<u8>,
        ram_bytes: usize,
        has_rumble: bool,
        has_battery: bool,
        persisted: Option<&BatteryState>,
    ) -> Result<Self, CoreError> {
        let ram = persisted_ram(persisted, ram_bytes, has_battery)?;
        if persisted.is_some_and(|state| !state.mapper_data().is_empty()) {
            return Err(CoreError::InvalidRom(
                "MBC5 persisted mapper data must be empty".into(),
            ));
        }
        let rom_banks = rom.len() / 0x4000;
        let external_ram_banks = ram.len().div_ceil(0x2000);
        Ok(Self {
            rom,
            ram,
            rom_banks,
            ram_banks: external_ram_banks,
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            has_rumble,
            has_battery,
        })
    }

    #[cfg(test)]
    pub(super) const fn selected_ram_bank(&self) -> u8 {
        self.ram_bank
    }
}

impl Mapper for Mbc5 {
    fn read_rom(&self, address: u16) -> u8 {
        let bank = if address < 0x4000 {
            0
        } else {
            usize::from(self.rom_bank) % self.rom_banks
        };
        rom_byte(&self.rom, bank, address)
    }
    fn write_rom(&mut self, address: u16, value: u8, _now: u64) {
        match address {
            0x0000..=0x1fff => self.ram_enabled = value & 0x0f == 0x0a,
            0x2000..=0x2fff => self.rom_bank = (self.rom_bank & 0x100) | u16::from(value),
            0x3000..=0x3fff => {
                self.rom_bank = (self.rom_bank & 0x0ff) | (u16::from(value & 1) << 8);
            }
            0x4000..=0x5fff => {
                let selected = value & if self.has_rumble { 0x07 } else { 0x0f };
                self.ram_bank = if self.ram_banks == 0 {
                    0
                } else {
                    u8::try_from(usize::from(selected) % self.ram_banks)
                        .expect("MBC5 supports at most sixteen RAM banks")
                };
            }
            _ => {}
        }
    }
    fn read_ram(&self, address: u16, _now: u64) -> u8 {
        if self.ram_enabled {
            ram_byte(&self.ram, usize::from(self.ram_bank), address)
        } else {
            0xff
        }
    }
    fn write_ram(&mut self, address: u16, value: u8, _now: u64) {
        if self.ram_enabled {
            write_ram_byte(&mut self.ram, usize::from(self.ram_bank), address, value);
        }
    }
    fn reset(&mut self, _now: u64) {
        self.ram_enabled = false;
        self.rom_bank = 1;
        self.ram_bank = 0;
    }
    fn battery_state(&self, _now: u64) -> Option<BatteryState> {
        self.has_battery
            .then(|| BatteryState::new(1, self.ram.clone(), Vec::new()))
    }
}
