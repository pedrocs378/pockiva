mod devices;
mod dma;

use std::num::NonZeroU32;

use crate::cartridge::Cartridge;
use crate::cpu::CpuBus;
use crate::interrupts::{Interrupt, InterruptMask, InterruptRegisters};
use crate::joypad::JoypadRegister;
use crate::timer::Timer;
use crate::{AudioBatch, BatteryState, Frame, JoypadState};
use devices::{AudioDevice, AudioRegisters, VideoDevice, VideoRegisters};
use dma::OamDma;

#[derive(Default)]
struct SerialPort {
    data: u8,
    control: u8,
    captured: Vec<u8>,
}

impl SerialPort {
    fn read(&self, address: u16) -> u8 {
        match address {
            0xff01 => self.data,
            0xff02 => self.control | 0x7e,
            _ => 0xff,
        }
    }
    fn write(&mut self, address: u16, value: u8) -> InterruptMask {
        match address {
            0xff01 => self.data = value,
            0xff02 => {
                self.control = value;
                if value == 0x81 {
                    if self.captured.len() == 4096 {
                        self.captured.remove(0);
                    }
                    self.captured.push(self.data);
                    self.control &= 0x7f;
                    return InterruptMask::from_bits(Interrupt::Serial.bit());
                }
            }
            _ => {}
        }
        InterruptMask::default()
    }
}

pub(crate) struct MachineBus {
    cartridge: Cartridge,
    wram: Box<[u8; 0x2000]>,
    hram: Box<[u8; 0x7f]>,
    joypad: JoypadRegister,
    timer: Timer,
    interrupts: InterruptRegisters,
    dma: OamDma,
    video: Box<dyn VideoDevice>,
    audio: Box<dyn AudioDevice>,
    serial: SerialPort,
    sample_rate: NonZeroU32,
    now_unix_seconds: u64,
    elapsed_t_cycles: u64,
}

impl MachineBus {
    pub(crate) fn new(cartridge: Cartridge, sample_rate: NonZeroU32, now: u64) -> Self {
        Self {
            cartridge,
            wram: Box::new([0; 0x2000]),
            hram: Box::new([0; 0x7f]),
            joypad: JoypadRegister::default(),
            timer: Timer::default(),
            interrupts: InterruptRegisters::default(),
            dma: OamDma::default(),
            video: Box::new(VideoRegisters::default()),
            audio: Box::new(AudioRegisters::new(sample_rate)),
            serial: SerialPort::default(),
            sample_rate,
            now_unix_seconds: now,
            elapsed_t_cycles: 0,
        }
    }

    fn tick_m_cycle(&mut self) {
        self.elapsed_t_cycles += 4;
        let mut requested = self.timer.tick(4);
        if let Some((source, index)) = self.dma.next_address() {
            let value = self.read_unclocked(source);
            self.video.dma_write_oam(index, value);
            self.dma.advance();
        }
        requested = requested.union(self.video.tick(4).requested_interrupts);
        requested = requested.union(self.audio.tick(4).requested_interrupts);
        self.interrupts.request(requested);
    }

    fn read_unclocked(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x7fff | 0xa000..=0xbfff => {
                self.cartridge.read(address, self.now_unix_seconds)
            }
            0x8000..=0x9fff | 0xfe00..=0xfe9f | 0xff40..=0xff4b => self.video.read(address),
            0xc000..=0xdfff => self.wram[usize::from(address - 0xc000)],
            0xe000..=0xfdff => self.wram[usize::from(address - 0xe000)],
            0xff00 => self.joypad.read(),
            0xff01..=0xff02 => self.serial.read(address),
            0xff04..=0xff07 => self.timer.read(address),
            0xff0f => self.interrupts.read_if(),
            0xff10..=0xff3f => self.audio.read(address),
            0xff80..=0xfffe => self.hram[usize::from(address - 0xff80)],
            0xffff => self.interrupts.read_ie(),
            _ => 0xff,
        }
    }

    fn write_unclocked(&mut self, address: u16, value: u8) {
        let requested = match address {
            0x0000..=0x7fff | 0xa000..=0xbfff => {
                self.cartridge.write(address, value, self.now_unix_seconds);
                InterruptMask::default()
            }
            0x8000..=0x9fff | 0xfe00..=0xfe9f | 0xff40..=0xff4b => {
                self.video.write(address, value);
                if address == 0xff46 {
                    self.dma.start(value);
                }
                InterruptMask::default()
            }
            0xc000..=0xdfff => {
                self.wram[usize::from(address - 0xc000)] = value;
                InterruptMask::default()
            }
            0xe000..=0xfdff => {
                self.wram[usize::from(address - 0xe000)] = value;
                InterruptMask::default()
            }
            0xff00 => self.joypad.write(value),
            0xff01..=0xff02 => self.serial.write(address, value),
            0xff04..=0xff07 => self.timer.write(address, value),
            0xff0f => {
                self.interrupts.write_if(value);
                InterruptMask::default()
            }
            0xff10..=0xff3f => {
                self.audio.write(address, value);
                InterruptMask::default()
            }
            0xff80..=0xfffe => {
                self.hram[usize::from(address - 0xff80)] = value;
                InterruptMask::default()
            }
            0xffff => {
                self.interrupts.write_ie(value);
                InterruptMask::default()
            }
            _ => InterruptMask::default(),
        };
        self.interrupts.request(requested);
    }

    pub(crate) fn set_unix_seconds(&mut self, now: u64) {
        self.now_unix_seconds = now;
    }
    pub(crate) fn set_joypad_state(&mut self, state: JoypadState) {
        let requested = self.joypad.set_state(state);
        self.interrupts.request(requested);
    }
    pub(crate) fn reset(&mut self, now: u64, state: JoypadState) {
        self.cartridge.reset(now);
        self.wram.fill(0);
        self.hram.fill(0);
        self.joypad = JoypadRegister::default();
        self.timer = Timer::default();
        self.interrupts = InterruptRegisters::default();
        self.dma = OamDma::default();
        self.video = Box::new(VideoRegisters::default());
        self.audio = Box::new(AudioRegisters::new(self.sample_rate));
        self.serial = SerialPort::default();
        self.now_unix_seconds = now;
        self.elapsed_t_cycles = 0;
        self.set_joypad_state(state);
    }
    pub(crate) fn frame_ready(&self) -> bool {
        self.video.frame_ready()
    }
    pub(crate) fn take_frame(&mut self) -> Option<Frame> {
        self.video.take_frame()
    }
    pub(crate) fn stereo_frames_available(&self) -> usize {
        self.audio.stereo_frames_available()
    }
    pub(crate) fn drain_audio(&mut self) -> AudioBatch {
        self.audio.drain_audio()
    }
    pub(crate) fn battery_state(&self, now: u64) -> Option<BatteryState> {
        self.cartridge.battery_state(now)
    }

    #[cfg(test)]
    pub(crate) fn serial_output(&self) -> &[u8] {
        &self.serial.captured
    }
}

impl CpuBus for MachineBus {
    fn read8(&mut self, address: u16) -> u8 {
        let value = if self.dma.active() && !(0xff80..=0xfffe).contains(&address) {
            0xff
        } else {
            self.read_unclocked(address)
        };
        self.tick_m_cycle();
        value
    }

    fn write8(&mut self, address: u16, value: u8) {
        let allowed =
            !self.dma.active() || (0xff80..=0xfffe).contains(&address) || address == 0xff46;
        let timer_write = allowed && (0xff04..=0xff07).contains(&address);
        if timer_write {
            self.write_unclocked(address, value);
        }
        self.tick_m_cycle();
        if allowed && !timer_write {
            self.write_unclocked(address, value);
        }
    }

    fn idle_m_cycle(&mut self) {
        self.tick_m_cycle();
    }
    fn peek8(&self, address: u16) -> u8 {
        if self.dma.active() && !(0xff80..=0xfffe).contains(&address) {
            0xff
        } else {
            self.read_unclocked(address)
        }
    }
    fn elapsed_t_cycles(&self) -> u64 {
        self.elapsed_t_cycles
    }
    fn pending_interrupts(&self) -> InterruptMask {
        self.interrupts.pending()
    }
    fn acknowledge_interrupt(&mut self, interrupt: Interrupt) {
        self.interrupts.acknowledge(interrupt);
    }
    fn reset_divider(&mut self) {
        let requested = self.timer.write(0xff04, 0);
        self.interrupts.request(requested);
    }
}

#[cfg(test)]
mod tests;
