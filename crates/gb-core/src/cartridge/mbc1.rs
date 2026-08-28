use crate::{BatteryState, CoreError};

use super::mapper::{Mapper, persisted_ram, ram_byte, rom_byte, write_ram_byte};

pub(super) struct Mbc1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_banks: usize,
    ram_enabled: bool,
    rom_bank_low: u8,
    bank_high: u8,
    ram_banking_mode: bool,
    has_battery: bool,
}

impl Mbc1 {
    pub(super) fn new(
        rom: Vec<u8>,
        ram_bytes: usize,
        has_battery: bool,
        persisted: Option<&BatteryState>,
    ) -> Result<Self, CoreError> {
        let ram = persisted_ram(persisted, ram_bytes, has_battery)?;
        if persisted.is_some_and(|state| !state.mapper_data().is_empty()) {
            return Err(CoreError::InvalidRom(
                "MBC1 persisted mapper data must be empty".into(),
            ));
        }
        let rom_banks = rom.len() / 0x4000;
        Ok(Self {
            rom,
            ram,
            rom_banks,
            ram_enabled: false,
            rom_bank_low: 1,
            bank_high: 0,
            ram_banking_mode: false,
            has_battery,
        })
    }

    fn fixed_bank(&self) -> usize {
        if self.ram_banking_mode {
            (usize::from(self.bank_high) << 5) % self.rom_banks
        } else {
            0
        }
    }

    fn switch_bank(&self) -> usize {
        let mut bank = (usize::from(self.bank_high) << 5) | usize::from(self.rom_bank_low);
        if bank.trailing_zeros() >= 5 {
            bank += 1;
        }
        bank % self.rom_banks
    }

    fn ram_bank(&self) -> usize {
        if self.ram_banking_mode {
            usize::from(self.bank_high)
        } else {
            0
        }
    }
}

impl Mapper for Mbc1 {
    fn read_rom(&self, address: u16) -> u8 {
        let bank = if address < 0x4000 {
            self.fixed_bank()
        } else {
            self.switch_bank()
        };
        rom_byte(&self.rom, bank, address)
    }

    fn write_rom(&mut self, address: u16, value: u8, _now: u64) {
        match address {
            0x0000..=0x1fff => self.ram_enabled = value & 0x0f == 0x0a,
            0x2000..=0x3fff => self.rom_bank_low = value & 0x1f,
            0x4000..=0x5fff => self.bank_high = value & 0x03,
            0x6000..=0x7fff => self.ram_banking_mode = value & 1 != 0,
            _ => {}
        }
    }

    fn read_ram(&self, address: u16, _now: u64) -> u8 {
        if self.ram_enabled {
            ram_byte(&self.ram, self.ram_bank(), address)
        } else {
            0xff
        }
    }

    fn write_ram(&mut self, address: u16, value: u8, _now: u64) {
        if self.ram_enabled {
            let bank = self.ram_bank();
            write_ram_byte(&mut self.ram, bank, address, value);
        }
    }

    fn reset(&mut self, _now: u64) {
        self.ram_enabled = false;
        self.rom_bank_low = 1;
        self.bank_high = 0;
        self.ram_banking_mode = false;
    }

    fn battery_state(&self, _now: u64) -> Option<BatteryState> {
        self.has_battery
            .then(|| BatteryState::new(1, self.ram.clone(), Vec::new()))
    }
}
