use crate::{BatteryState, CoreError};

use super::mapper::{Mapper, persisted_ram, ram_byte, rom_byte, write_ram_byte};

const RTC_PERIOD_SECONDS: u64 = 512 * 24 * 60 * 60;

#[derive(Clone)]
struct RtcState {
    counter_seconds: u64,
    last_update_unix: u64,
    halted: bool,
    carry: bool,
    latched: [u8; 5],
    latch_armed: bool,
}

impl RtcState {
    fn new(now: u64) -> Self {
        let mut rtc = Self {
            counter_seconds: 0,
            last_update_unix: now,
            halted: false,
            carry: false,
            latched: [0; 5],
            latch_armed: false,
        };
        rtc.latched = rtc.registers(now);
        rtc
    }

    fn from_bytes(bytes: &[u8], now: u64) -> Result<Self, CoreError> {
        if bytes.len() != 22 || &bytes[..4] != b"M3R1" {
            return Err(CoreError::InvalidRom(
                "invalid MBC3 persisted-state schema".into(),
            ));
        }
        let counter_seconds = u64::from_le_bytes(bytes[4..12].try_into().expect("fixed slice"));
        let last_update_unix = u64::from_le_bytes(bytes[12..20].try_into().expect("fixed slice"));
        if counter_seconds >= RTC_PERIOD_SECONDS || bytes[20] > 1 || bytes[21] > 1 {
            return Err(CoreError::InvalidRom(
                "invalid MBC3 persisted-state values".into(),
            ));
        }
        let mut rtc = Self {
            counter_seconds,
            last_update_unix,
            halted: bytes[20] != 0,
            carry: bytes[21] != 0,
            latched: [0; 5],
            latch_armed: false,
        };
        rtc.sync(now);
        rtc.latched = rtc.registers(now);
        Ok(rtc)
    }

    fn live(&self, now: u64) -> (u64, bool) {
        if self.halted || now < self.last_update_unix {
            return (self.counter_seconds, self.carry);
        }
        let total = self.counter_seconds + (now - self.last_update_unix);
        (
            total % RTC_PERIOD_SECONDS,
            self.carry || total >= RTC_PERIOD_SECONDS,
        )
    }

    fn sync(&mut self, now: u64) {
        let (counter, carry) = self.live(now);
        self.counter_seconds = counter;
        self.carry = carry;
        if now >= self.last_update_unix {
            self.last_update_unix = now;
        }
    }

    fn registers(&self, now: u64) -> [u8; 5] {
        let (counter, carry) = self.live(now);
        let days = counter / 86_400;
        let in_day = counter % 86_400;
        [
            u8::try_from(in_day % 60).expect("seconds are below 60"),
            u8::try_from((in_day / 60) % 60).expect("minutes are below 60"),
            u8::try_from(in_day / 3_600).expect("hours are below 24"),
            days.to_le_bytes()[0],
            (days.to_le_bytes()[1] & 1) | (u8::from(self.halted) << 6) | (u8::from(carry) << 7),
        ]
    }

    fn latch(&mut self, value: u8, now: u64) {
        if value == 0 {
            self.latch_armed = true;
        } else if value == 1 && self.latch_armed {
            self.sync(now);
            self.latched = self.registers(now);
            self.latch_armed = false;
        } else {
            self.latch_armed = false;
        }
    }

    fn write_register(&mut self, selector: u8, value: u8, now: u64) {
        self.sync(now);
        let mut registers = self.registers(now);
        match selector {
            0x08 => registers[0] = value % 60,
            0x09 => registers[1] = value % 60,
            0x0a => registers[2] = value % 24,
            0x0b => registers[3] = value,
            0x0c => registers[4] = value & 0xc1,
            _ => return,
        }
        let days = u64::from(registers[3]) | (u64::from(registers[4] & 1) << 8);
        self.counter_seconds = days * 86_400
            + u64::from(registers[2]) * 3_600
            + u64::from(registers[1]) * 60
            + u64::from(registers[0]);
        let was_halted = self.halted;
        self.halted = registers[4] & 0x40 != 0;
        self.carry = registers[4] & 0x80 != 0;
        if was_halted != self.halted || !self.halted {
            self.last_update_unix = now;
        }
    }

    fn bytes(&self, now: u64) -> Vec<u8> {
        let (counter, carry) = self.live(now);
        let persisted_timestamp = self.last_update_unix.max(now);
        let mut bytes = Vec::with_capacity(22);
        bytes.extend_from_slice(b"M3R1");
        bytes.extend_from_slice(&counter.to_le_bytes());
        bytes.extend_from_slice(&persisted_timestamp.to_le_bytes());
        bytes.push(u8::from(self.halted));
        bytes.push(u8::from(carry));
        bytes
    }
}

pub(super) struct Mbc3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_banks: usize,
    ram_enabled: bool,
    rom_bank: u8,
    selector: u8,
    rtc: Option<RtcState>,
    has_battery: bool,
}

impl Mbc3 {
    pub(super) fn new(
        rom: Vec<u8>,
        ram_bytes: usize,
        has_battery: bool,
        has_timer: bool,
        persisted: Option<&BatteryState>,
        now: u64,
    ) -> Result<Self, CoreError> {
        let ram = persisted_ram(persisted, ram_bytes, has_battery)?;
        let rtc = if has_timer {
            Some(match persisted {
                Some(state) => RtcState::from_bytes(state.mapper_data(), now)?,
                None => RtcState::new(now),
            })
        } else {
            if persisted.is_some_and(|state| !state.mapper_data().is_empty()) {
                return Err(CoreError::InvalidRom(
                    "non-timer MBC3 mapper data must be empty".into(),
                ));
            }
            None
        };
        let rom_banks = rom.len() / 0x4000;
        Ok(Self {
            rom,
            ram,
            rom_banks,
            ram_enabled: false,
            rom_bank: 1,
            selector: 0,
            rtc,
            has_battery,
        })
    }
}

impl Mapper for Mbc3 {
    fn read_rom(&self, address: u16) -> u8 {
        let bank = if address < 0x4000 {
            0
        } else {
            usize::from(self.rom_bank) % self.rom_banks
        };
        rom_byte(&self.rom, bank, address)
    }

    fn write_rom(&mut self, address: u16, value: u8, now: u64) {
        match address {
            0x0000..=0x1fff => self.ram_enabled = value & 0x0f == 0x0a,
            0x2000..=0x3fff => {
                self.rom_bank = value & 0x7f;
                if self.rom_bank == 0 {
                    self.rom_bank = 1;
                }
            }
            0x4000..=0x5fff => self.selector = value,
            0x6000..=0x7fff => {
                if let Some(rtc) = self.rtc.as_mut() {
                    rtc.latch(value, now);
                }
            }
            _ => {}
        }
    }

    fn read_ram(&self, address: u16, _now: u64) -> u8 {
        if !self.ram_enabled {
            return 0xff;
        }
        match self.selector {
            0x00..=0x03 => ram_byte(&self.ram, usize::from(self.selector), address),
            0x08..=0x0c => self
                .rtc
                .as_ref()
                .map_or(0xff, |rtc| rtc.latched[usize::from(self.selector - 0x08)]),
            _ => 0xff,
        }
    }

    fn write_ram(&mut self, address: u16, value: u8, now: u64) {
        if !self.ram_enabled {
            return;
        }
        match self.selector {
            0x00..=0x03 => {
                write_ram_byte(&mut self.ram, usize::from(self.selector), address, value);
            }
            0x08..=0x0c => {
                if let Some(rtc) = self.rtc.as_mut() {
                    rtc.write_register(self.selector, value, now);
                }
            }
            _ => {}
        }
    }

    fn reset(&mut self, now: u64) {
        self.ram_enabled = false;
        self.rom_bank = 1;
        self.selector = 0;
        if let Some(rtc) = self.rtc.as_mut() {
            rtc.sync(now);
            rtc.latch_armed = false;
        }
    }

    fn battery_state(&self, now: u64) -> Option<BatteryState> {
        self.has_battery.then(|| {
            let mapper_data = self
                .rtc
                .as_ref()
                .map_or_else(Vec::new, |rtc| rtc.bytes(now));
            BatteryState::new(1, self.ram.clone(), mapper_data)
        })
    }
}
