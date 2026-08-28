//! Deterministic original-DMG audio processing unit.

use std::num::NonZeroU32;

use crate::{AudioBatch, DMG_CLOCK_HZ};

mod envelope;
mod length;
mod mixer;
mod noise;
mod pcm;
mod pulse;
mod sweep;
mod wave;

use mixer::Mixer;
use noise::NoiseChannel;
use pcm::PcmBuffer;
use pulse::PulseChannel;
use wave::WaveChannel;

pub(crate) struct Apu {
    sample_rate: NonZeroU32,
    master_enabled: bool,
    divider_mirror: u16,
    frame_step: u8,
    nr50: u8,
    nr51: u8,
    sample_phase: u64,
    #[cfg(test)]
    generated_stereo_frames: u64,
    channel1: PulseChannel,
    channel2: PulseChannel,
    channel3: WaveChannel,
    channel4: NoiseChannel,
    mixer: Mixer,
    pcm: PcmBuffer,
}

impl Apu {
    pub(crate) fn new(sample_rate: NonZeroU32) -> Self {
        let mut apu = Self {
            sample_rate,
            master_enabled: true,
            divider_mirror: 0xabcc,
            frame_step: 0,
            nr50: 0,
            nr51: 0,
            sample_phase: 0,
            #[cfg(test)]
            generated_stereo_frames: 0,
            channel1: PulseChannel::new(true),
            channel2: PulseChannel::new(false),
            channel3: WaveChannel::default(),
            channel4: NoiseChannel::default(),
            mixer: Mixer::new(sample_rate),
            pcm: PcmBuffer::default(),
        };
        // The public machine starts after the DMG boot ROM, so preserve the
        // documented post-boot mixer and channel-one register state.
        apu.channel1.write(0, 0x00, true);
        apu.channel1.write(1, 0x80, true);
        apu.channel1.write(2, 0xf3, true);
        apu.channel1.write(3, 0x00, true);
        apu.channel1.write(4, 0x80, true);
        apu.nr50 = 0x77;
        apu.nr51 = 0xf3;
        apu
    }

    pub(crate) fn read(&self, address: u16) -> u8 {
        match address {
            0xff10..=0xff14 => self.channel1.read(Self::register_offset(address, 0xff10)),
            0xff16..=0xff19 => self.channel2.read(Self::register_offset(address, 0xff15)),
            0xff1a..=0xff1e => self.channel3.read(Self::register_offset(address, 0xff1a)),
            0xff20..=0xff23 => self.channel4.read(Self::register_offset(address, 0xff20)),
            0xff24 => self.nr50,
            0xff25 => self.nr51,
            0xff26 => {
                0x70 | (u8::from(self.master_enabled) << 7)
                    | u8::from(self.channel1.active())
                    | (u8::from(self.channel2.active()) << 1)
                    | (u8::from(self.channel3.active()) << 2)
                    | (u8::from(self.channel4.active()) << 3)
            }
            0xff30..=0xff3f => self.channel3.read_wave_ram(address),
            _ => 0xff,
        }
    }

    pub(crate) fn write(&mut self, address: u16, value: u8) {
        if address == 0xff04 {
            if self.divider_mirror & 0x1000 != 0 {
                self.clock_frame_sequencer();
            }
            self.divider_mirror = 0;
            return;
        }
        if address == 0xff26 {
            self.write_nr52(value);
            return;
        }
        if !self.master_enabled {
            let next_length = self.next_step_clocks_length();
            match address {
                0xff11 => self.channel1.write_length_while_powered_off(value),
                0xff16 => self.channel2.write_length_while_powered_off(value),
                0xff1b => self.channel3.write(1, value, next_length),
                0xff20 => self.channel4.write(0, value, next_length),
                0xff30..=0xff3f => self.channel3.write_wave_ram(address, value),
                _ => {}
            }
            return;
        }
        let next_length = self.next_step_clocks_length();
        match address {
            0xff10..=0xff14 => {
                self.channel1
                    .write(Self::register_offset(address, 0xff10), value, next_length);
            }
            0xff16..=0xff19 => {
                self.channel2
                    .write(Self::register_offset(address, 0xff15), value, next_length);
            }
            0xff1a..=0xff1e => {
                self.channel3
                    .write(Self::register_offset(address, 0xff1a), value, next_length);
            }
            0xff20..=0xff23 => {
                self.channel4
                    .write(Self::register_offset(address, 0xff20), value, next_length);
            }
            0xff24 => self.nr50 = value,
            0xff25 => self.nr51 = value,
            0xff30..=0xff3f => self.channel3.write_wave_ram(address, value),
            _ => {}
        }
    }

    fn register_offset(address: u16, base: u16) -> u8 {
        u8::try_from(address - base).expect("APU register range has an eight-bit offset")
    }

    fn write_nr52(&mut self, value: u8) {
        let enable = value & 0x80 != 0;
        if self.master_enabled && !enable {
            self.master_enabled = false;
            self.nr50 = 0;
            self.nr51 = 0;
            self.frame_step = 0;
            self.channel1.power_off();
            self.channel2.power_off();
            self.channel3.power_off_preserving_ram();
            self.channel4.power_off();
            self.mixer.reset();
            self.pcm.clear();
        } else if !self.master_enabled && enable {
            self.master_enabled = true;
            self.frame_step = 0;
        }
    }

    #[cfg(test)]
    pub(crate) fn reset(&mut self) {
        *self = Self::new(self.sample_rate);
    }

    pub(crate) fn tick_t_cycle(&mut self) {
        let old_high = self.divider_mirror & 0x1000 != 0;
        self.divider_mirror = self.divider_mirror.wrapping_add(1);
        if old_high && self.divider_mirror & 0x1000 == 0 {
            self.clock_frame_sequencer();
        }
        if self.master_enabled {
            self.channel1.tick_t_cycle();
            self.channel2.tick_t_cycle();
            self.channel3.tick_t_cycle();
            self.channel4.tick_t_cycle();
        }
        self.sample_phase += u64::from(self.sample_rate.get());
        while self.sample_phase >= u64::from(DMG_CLOCK_HZ) {
            self.sample_phase -= u64::from(DMG_CLOCK_HZ);
            let samples = [
                self.channel1.output(),
                self.channel2.output(),
                self.channel3.output(),
                self.channel4.output(),
            ];
            let dacs = [
                self.channel1.dac_enabled(),
                self.channel2.dac_enabled(),
                self.channel3.dac_enabled(),
                self.channel4.dac_enabled(),
            ];
            let [left, right] = if self.master_enabled {
                self.mixer
                    .mix_with_dacs(samples, dacs, self.nr50, self.nr51)
            } else {
                [0.0, 0.0]
            };
            self.pcm.push_stereo(left, right);
            #[cfg(test)]
            {
                self.generated_stereo_frames += 1;
            }
        }
    }

    fn clock_frame_sequencer(&mut self) {
        match self.frame_step {
            0 | 4 => self.clock_lengths(),
            2 | 6 => {
                self.clock_lengths();
                self.channel1.clock_sweep();
            }
            7 => {
                self.channel1.clock_envelope();
                self.channel2.clock_envelope();
                self.channel4.clock_envelope();
            }
            _ => {}
        }
        self.frame_step = (self.frame_step + 1) & 7;
    }

    fn clock_lengths(&mut self) {
        self.channel1.clock_length();
        self.channel2.clock_length();
        self.channel3.clock_length();
        self.channel4.clock_length();
    }

    const fn next_step_clocks_length(&self) -> bool {
        self.frame_step & 1 == 0
    }

    pub(crate) fn stereo_frames_available(&self) -> usize {
        self.pcm.stereo_frames_available()
    }
    pub(crate) fn drain_audio(&mut self) -> AudioBatch {
        self.pcm.drain(self.sample_rate)
    }

    #[cfg(test)]
    pub(crate) const fn divider_mirror_for_test(&self) -> u16 {
        self.divider_mirror
    }
    #[cfg(test)]
    pub(crate) const fn frame_step_for_test(&self) -> u8 {
        self.frame_step
    }
    #[cfg(test)]
    pub(crate) const fn generated_stereo_frames_for_test(&self) -> u64 {
        self.generated_stereo_frames
    }
    #[cfg(test)]
    pub(crate) const fn dropped_stereo_frames_for_test(&self) -> u64 {
        self.pcm.dropped_stereo_frames()
    }
}

#[cfg(test)]
mod tests;
