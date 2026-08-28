use crate::{BatteryState, CoreError};

use super::mapper::{Mapper, persisted_ram, ram_byte, write_ram_byte};

pub(super) struct RomOnly {
    rom: Vec<u8>,
    ram: Vec<u8>,
    has_battery: bool,
}

impl RomOnly {
    pub(super) fn new(
        rom: Vec<u8>,
        ram_bytes: usize,
        has_battery: bool,
        persisted: Option<&BatteryState>,
    ) -> Result<Self, CoreError> {
        let ram = persisted_ram(persisted, ram_bytes, has_battery)?;
        if let Some(state) = persisted {
            if !state.mapper_data().is_empty() {
                return Err(CoreError::InvalidRom(
                    "ROM-only persisted mapper data must be empty".into(),
                ));
            }
        }
        Ok(Self {
            rom,
            ram,
            has_battery,
        })
    }
}

impl Mapper for RomOnly {
    fn read_rom(&self, address: u16) -> u8 {
        self.rom.get(usize::from(address)).copied().unwrap_or(0xff)
    }
    fn write_rom(&mut self, _address: u16, _value: u8, _now: u64) {}
    fn read_ram(&self, address: u16, _now: u64) -> u8 {
        ram_byte(&self.ram, 0, address)
    }
    fn write_ram(&mut self, address: u16, value: u8, _now: u64) {
        write_ram_byte(&mut self.ram, 0, address, value);
    }
    fn reset(&mut self, _now: u64) {}
    fn battery_state(&self, _now: u64) -> Option<BatteryState> {
        self.has_battery
            .then(|| BatteryState::new(1, self.ram.clone(), Vec::new()))
    }
}
