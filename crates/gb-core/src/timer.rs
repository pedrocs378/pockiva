use crate::interrupts::{Interrupt, InterruptMask};

#[derive(Debug, Clone)]
pub(crate) struct Timer {
    divider: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    reload_delay: Option<u8>,
    reloaded_this_cycle: bool,
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            divider: 0xabcc,
            tima: 0,
            tma: 0,
            tac: 0,
            reload_delay: None,
            reloaded_this_cycle: false,
        }
    }
}

impl Timer {
    pub(crate) fn read(&self, address: u16) -> u8 {
        match address {
            0xff04 => (self.divider >> 8) as u8,
            0xff05 => self.tima,
            0xff06 => self.tma,
            0xff07 => 0xf8 | self.tac,
            _ => 0xff,
        }
    }

    pub(crate) fn write(&mut self, address: u16, value: u8) -> InterruptMask {
        match address {
            0xff04 => {
                let old = self.timer_input();
                self.divider = 0;
                if old && !self.timer_input() {
                    self.increment_tima();
                }
            }
            0xff05 if !self.reloaded_this_cycle => {
                self.tima = value;
                self.reload_delay = None;
            }
            0xff06 => {
                self.tma = value;
                if self.reloaded_this_cycle {
                    self.tima = value;
                }
            }
            0xff07 => {
                let old = self.timer_input();
                self.tac = value & 0x07;
                if old && !self.timer_input() {
                    self.increment_tima();
                }
            }
            _ => {}
        }
        InterruptMask::default()
    }

    pub(crate) fn tick(&mut self, t_cycles: u32) -> InterruptMask {
        let mut requested = InterruptMask::default();
        for _ in 0..t_cycles {
            self.reloaded_this_cycle = false;
            if let Some(delay) = self.reload_delay {
                if delay == 1 {
                    self.reload_delay = None;
                    self.tima = self.tma;
                    self.reloaded_this_cycle = true;
                    requested = requested.union(InterruptMask::from_bits(Interrupt::Timer.bit()));
                } else {
                    self.reload_delay = Some(delay - 1);
                }
            }
            let old = self.timer_input();
            self.divider = self.divider.wrapping_add(1);
            if old && !self.timer_input() {
                self.increment_tima();
            }
        }
        requested
    }

    fn timer_input(&self) -> bool {
        if self.tac & 0x04 == 0 {
            return false;
        }
        let bit = match self.tac & 0x03 {
            0 => 9,
            1 => 3,
            2 => 5,
            _ => 7,
        };
        self.divider & (1 << bit) != 0
    }

    fn increment_tima(&mut self) {
        if self.reload_delay.is_some() {
            return;
        }
        if self.tima == 0xff {
            self.tima = 0;
            self.reload_delay = Some(4);
        } else {
            self.tima = self.tima.wrapping_add(1);
        }
    }

    #[cfg(test)]
    fn for_test(counter: u8, modulo: u8, control: u8) -> Self {
        Self {
            divider: 0,
            tima: counter,
            tma: modulo,
            tac: control,
            reload_delay: None,
            reloaded_this_cycle: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Timer;
    use crate::interrupts::{Interrupt, InterruptMask};

    #[test]
    fn tac_selects_divider_bits_9_3_5_7() {
        for (selector, period) in [(0_u8, 1024_u32), (1, 16), (2, 64), (3, 256)] {
            let mut timer = Timer::for_test(0, 0, 0b100 | selector);
            timer.tick(period - 1);
            assert_eq!(timer.read(0xff05), 0);
            timer.tick(1);
            assert_eq!(timer.read(0xff05), 1);
        }
    }

    #[test]
    fn tima_overflow_reloads_after_four_t_cycles_and_requests_interrupt() {
        let mut timer = Timer::for_test(0xff, 0xa5, 0b101);
        assert_eq!(timer.tick(16), InterruptMask::default());
        assert_eq!(timer.read(0xff05), 0);
        assert_eq!(
            timer.tick(4),
            InterruptMask::from_bits(Interrupt::Timer.bit())
        );
        assert_eq!(timer.read(0xff05), 0xa5);
    }

    #[test]
    fn tima_write_cancels_pending_reload() {
        let mut timer = Timer::for_test(0xff, 0xa5, 0b101);
        timer.tick(16);
        timer.write(0xff05, 0x77);
        assert_eq!(timer.tick(4), InterruptMask::default());
        assert_eq!(timer.read(0xff05), 0x77);
    }

    #[test]
    fn div_and_tac_writes_observe_falling_edges() {
        let mut div = Timer::for_test(0, 0, 0b101);
        div.tick(8);
        div.write(0xff04, 0);
        assert_eq!(div.read(0xff05), 1);

        let mut tac = Timer::for_test(0, 0, 0b101);
        tac.tick(8);
        tac.write(0xff07, 0);
        assert_eq!(tac.read(0xff05), 1);
    }
}
