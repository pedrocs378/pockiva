use super::length::LengthCounter;

const CPU_READ_T_CYCLES: u16 = 4;
const APU_FETCH_T_CYCLES: u16 = 2;

#[derive(Debug, Clone, Default)]
pub(crate) struct WaveChannel {
    enabled: bool,
    dac_enabled: bool,
    frequency: u16,
    timer: u16,
    position: u8,
    sample_buffer: u8,
    output_level: u8,
    length: LengthCounter<256>,
    wave_ram: [u8; 16],
    access_window_t_cycles: u8,
}

impl WaveChannel {
    pub(crate) fn read(&self, register: u8) -> u8 {
        match register {
            0 => u8::from(self.dac_enabled) << 7 | 0x7f,
            2 => self.output_level << 5 | 0x9f,
            4 => 0xbf | (u8::from(self.length.enabled()) << 6),
            _ => 0xff,
        }
    }

    pub(crate) fn write(&mut self, register: u8, value: u8, next_step_clocks_length: bool) {
        match register {
            0 => {
                self.dac_enabled = value & 0x80 != 0;
                if !self.dac_enabled {
                    self.enabled = false;
                }
            }
            1 => self.length.load(value),
            2 => self.output_level = (value >> 5) & 0x03,
            3 => self.frequency = (self.frequency & 0x0700) | u16::from(value),
            4 => {
                self.frequency = (self.frequency & 0x00ff) | (u16::from(value & 7) << 8);
                if self
                    .length
                    .set_enabled(value & 0x40 != 0, next_step_clocks_length, self.enabled)
                {
                    self.enabled = false;
                }
                if value & 0x80 != 0 {
                    self.trigger(next_step_clocks_length);
                }
            }
            _ => {}
        }
    }

    fn trigger(&mut self, next_step_clocks_length: bool) {
        if self.enabled && self.timer == APU_FETCH_T_CYCLES {
            let byte_index = usize::from(((self.position + 1) & 31) / 2);
            if byte_index < 4 {
                self.wave_ram[0] = self.wave_ram[byte_index];
            } else {
                let start = byte_index & !3;
                self.wave_ram.copy_within(start..start + 4, 0);
            }
        }
        let _ = self.length.trigger(next_step_clocks_length);
        self.enabled = self.dac_enabled;
        self.position = 0;
        self.timer = self.period() + 6;
        self.access_window_t_cycles = 0;
    }

    pub(crate) fn tick_t_cycle(&mut self) {
        self.access_window_t_cycles = self.access_window_t_cycles.saturating_sub(1);
        self.timer = self.timer.saturating_sub(1);
        if self.timer == 0 {
            self.timer = self.period();
            self.position = (self.position + 1) & 31;
            self.sample_buffer = self.wave_ram[usize::from(self.position / 2)];
            self.access_window_t_cycles = 1;
        }
    }

    pub(crate) fn clock_length(&mut self) {
        if self.length.clock() {
            self.enabled = false;
        }
    }

    pub(crate) const fn output(&self) -> u8 {
        if !self.enabled || !self.dac_enabled || self.output_level == 0 {
            return 0;
        }
        let sample = if self.position & 1 == 0 {
            self.sample_buffer >> 4
        } else {
            self.sample_buffer & 0x0f
        };
        sample >> (self.output_level - 1)
    }

    pub(crate) fn read_wave_ram(&self, address: u16) -> u8 {
        if !self.enabled {
            self.wave_ram[usize::from(address - 0xff30)]
        } else if self.timer == CPU_READ_T_CYCLES {
            self.wave_ram[usize::from(((self.position + 1) & 31) / 2)]
        } else {
            0xff
        }
    }

    pub(crate) fn write_wave_ram(&mut self, address: u16, value: u8) {
        if !self.enabled {
            self.wave_ram[usize::from(address - 0xff30)] = value;
        } else if self.access_window_t_cycles > 0 {
            self.wave_ram[usize::from(self.position / 2)] = value;
        }
    }

    const fn period(&self) -> u16 {
        (2048 - self.frequency) * 2
    }

    pub(crate) const fn active(&self) -> bool {
        self.enabled
    }
    pub(crate) const fn dac_enabled(&self) -> bool {
        self.dac_enabled
    }

    pub(crate) fn power_off_preserving_ram(&mut self) {
        let wave_ram = self.wave_ram;
        *self = Self::default();
        self.wave_ram = wave_ram;
    }
}

#[cfg(test)]
mod tests {
    use super::WaveChannel;

    #[test]
    fn trigger_waits_six_extra_t_cycles_before_first_wave_ram_fetch() {
        let mut wave = WaveChannel::default();
        wave.write_wave_ram(0xff30, 0xf1);
        wave.write(0, 0x80, true);
        wave.write(2, 0x20, true);
        wave.write(3, 0xff, true);
        wave.write(4, 0x87, true);

        assert_eq!(wave.output(), 0);
        for elapsed in 1..8 {
            wave.tick_t_cycle();
            assert_eq!(
                wave.position, 0,
                "wave position must not advance after {elapsed} T-cycles"
            );
            assert_eq!(wave.output(), 0);
        }
        wave.tick_t_cycle();
        assert_eq!(wave.position, 1);
        assert_eq!(wave.output(), 1);
    }

    #[test]
    fn wave_ram_read_predicts_fetch_at_end_of_cpu_access() {
        let mut wave = WaveChannel::default();
        wave.write_wave_ram(0xff30, 0x12);
        wave.write(0, 0x80, true);
        wave.write(3, 0xfe, true);
        wave.write(4, 0x87, true);

        for _ in 0..6 {
            wave.tick_t_cycle();
        }
        assert_eq!(wave.read_wave_ram(0xff3f), 0x12);
        wave.tick_t_cycle();
        assert_eq!(wave.read_wave_ram(0xff3f), 0xff);
    }

    #[test]
    fn write_is_redirected_only_on_fetch_t_cycle() {
        let mut wave = WaveChannel::default();
        wave.write_wave_ram(0xff30, 0x12);
        wave.write_wave_ram(0xff3f, 0xfe);
        wave.write(0, 0x80, true);
        wave.write(3, 0xfe, true);
        wave.write(4, 0x87, true);

        for _ in 0..10 {
            wave.tick_t_cycle();
        }
        wave.write_wave_ram(0xff3f, 0xab);
        wave.write(0, 0, true);

        assert_eq!(wave.read_wave_ram(0xff30), 0xab);
        assert_eq!(wave.read_wave_ram(0xff3f), 0xfe);
    }

    #[test]
    fn retrigger_corrupts_from_byte_due_on_next_fetch() {
        let mut wave = WaveChannel::default();
        for (index, value) in (0x10..=0x1f).enumerate() {
            wave.write_wave_ram(
                0xff30 + u16::try_from(index).expect("wave RAM index fits in u16"),
                value,
            );
        }
        wave.write(0, 0x80, true);
        wave.write(3, 0xfe, true);
        wave.write(4, 0x87, true);

        for _ in 0..20 {
            wave.tick_t_cycle();
        }
        wave.write(4, 0x87, true);
        wave.write(0, 0, true);

        assert_eq!(wave.read_wave_ram(0xff30), 0x12);
    }
}
