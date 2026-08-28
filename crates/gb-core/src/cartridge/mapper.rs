use crate::BatteryState;

pub(crate) trait Mapper: Send {
    fn read_rom(&self, address: u16) -> u8;
    fn write_rom(&mut self, address: u16, value: u8, now: u64);
    fn read_ram(&self, address: u16, now: u64) -> u8;
    fn write_ram(&mut self, address: u16, value: u8, now: u64);
    fn reset(&mut self, now: u64);
    fn battery_state(&self, now: u64) -> Option<BatteryState>;
}

pub(super) fn persisted_ram(
    persisted: Option<&BatteryState>,
    expected_len: usize,
    has_battery: bool,
) -> Result<Vec<u8>, crate::CoreError> {
    match persisted {
        None => Ok(vec![0; expected_len]),
        Some(_) if !has_battery => Err(crate::CoreError::InvalidRom(
            "persisted state supplied for a cartridge without a battery".into(),
        )),
        Some(state) if state.format_version() != 1 => Err(crate::CoreError::InvalidRom(
            "unsupported persisted-state format version".into(),
        )),
        Some(state) if state.ram().len() != expected_len => Err(crate::CoreError::InvalidRom(
            "persisted RAM length does not match the cartridge header".into(),
        )),
        Some(state) => Ok(state.ram().to_vec()),
    }
}

pub(super) fn rom_byte(rom: &[u8], bank: usize, address: u16) -> u8 {
    let offset = bank * 0x4000 + usize::from(address & 0x3fff);
    rom.get(offset).copied().unwrap_or(0xff)
}

pub(super) fn ram_byte(ram: &[u8], bank: usize, address: u16) -> u8 {
    let offset = bank * 0x2000 + usize::from(address - 0xa000);
    ram.get(offset).copied().unwrap_or(0xff)
}

pub(super) fn write_ram_byte(ram: &mut [u8], bank: usize, address: u16, value: u8) {
    let offset = bank * 0x2000 + usize::from(address - 0xa000);
    if let Some(byte) = ram.get_mut(offset) {
        *byte = value;
    }
}
