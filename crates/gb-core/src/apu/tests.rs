use std::num::NonZeroU32;

use super::Apu;
use super::envelope::VolumeEnvelope;
use super::length::LengthCounter;
use super::mixer::Mixer;
use super::noise::{NoiseChannel, noise_period};
use super::pcm::{MAX_CORE_STEREO_FRAMES, PcmBuffer};
use super::pulse::PulseChannel;
use super::sweep::{FrequencySweep, SweepClock, SweepTrigger};
use super::wave::WaveChannel;
use crate::DMG_CLOCK_HZ;

fn rate(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).expect("non-zero test rate")
}
fn test_apu() -> Apu {
    Apu::new(rate(48_000))
}

fn assert_stereo_close(actual: [f32; 2], expected: [f32; 2], context: &str) {
    for side in 0..2 {
        assert!(
            (actual[side] - expected[side]).abs() <= f32::EPSILON,
            "{context}, side {side}: expected {}, got {}",
            expected[side],
            actual[side]
        );
    }
}

#[test]
fn post_boot_register_state_matches_original_dmg() {
    let apu = test_apu();
    for (address, expected) in [
        (0xff10, 0x80),
        (0xff11, 0xbf),
        (0xff12, 0xf3),
        (0xff14, 0xbf),
        (0xff16, 0x3f),
        (0xff17, 0x00),
        (0xff19, 0xbf),
        (0xff1a, 0x7f),
        (0xff1c, 0x9f),
        (0xff1e, 0xbf),
        (0xff21, 0x00),
        (0xff22, 0x00),
        (0xff23, 0xbf),
        (0xff24, 0x77),
        (0xff25, 0xf3),
        (0xff26, 0xf1),
    ] {
        assert_eq!(apu.read(address), expected, "address {address:#06x}");
    }
}

#[test]
fn register_reads_apply_dmg_masks() {
    let mut apu = test_apu();
    for (address, written, expected) in [
        (0xff10, 0x7f, 0xff),
        (0xff11, 0x80, 0xbf),
        (0xff12, 0xa5, 0xa5),
        (0xff13, 0x00, 0xff),
        (0xff14, 0x40, 0xff),
        (0xff16, 0x40, 0x7f),
        (0xff17, 0x5a, 0x5a),
        (0xff1a, 0x80, 0xff),
        (0xff1b, 0x00, 0xff),
        (0xff1c, 0x60, 0xff),
        (0xff20, 0x00, 0xff),
        (0xff21, 0xf3, 0xf3),
        (0xff22, 0x35, 0x35),
        (0xff23, 0x40, 0xff),
        (0xff24, 0x77, 0x77),
        (0xff25, 0xf3, 0xf3),
        (0xff27, 0x00, 0xff),
        (0xff2f, 0x00, 0xff),
    ] {
        apu.write(address, written);
        assert_eq!(apu.read(address), expected, "address {address:#06x}");
    }
    assert_eq!(apu.read(0xff15), 0xff);
    assert_eq!(apu.read(0xff1f), 0xff);
    assert_eq!(apu.read(0xc000), 0xff);
    apu.write(0xff30, 0x12);
    apu.write(0xff3f, 0xfe);
    assert_eq!(apu.read(0xff30), 0x12);
    assert_eq!(apu.read(0xff3f), 0xfe);
}

#[test]
fn register_power_off_clears_state_but_preserves_wave_ram_and_lengths() {
    let mut apu = test_apu();
    apu.write(0xff12, 0xf3);
    apu.write(0xff24, 0x77);
    apu.write(0xff30, 0xa5);
    apu.write(0xff26, 0);
    assert_eq!(apu.read(0xff26), 0x70);
    assert_eq!(apu.read(0xff12), 0);
    assert_eq!(apu.read(0xff24), 0);
    assert_eq!(apu.read(0xff30), 0xa5);
    apu.write(0xff12, 0xff);
    assert_eq!(apu.read(0xff12), 0);
    apu.write(0xff11, 0xc0);
    assert_eq!(apu.read(0xff11), 0x3f);
    apu.write(0xff26, 0x80);
    assert_eq!(apu.read(0xff26), 0xf0);
}

#[test]
fn every_cpu_visible_register_obeys_powered_on_and_powered_off_masks() {
    let powered_on = [
        (0xff10, 0xda),
        (0xff11, 0x7f),
        (0xff12, 0x5a),
        (0xff13, 0xff),
        (0xff14, 0xff),
        (0xff15, 0xff),
        (0xff16, 0x7f),
        (0xff17, 0x5a),
        (0xff18, 0xff),
        (0xff19, 0xff),
        (0xff1a, 0x7f),
        (0xff1b, 0xff),
        (0xff1c, 0xdf),
        (0xff1d, 0xff),
        (0xff1e, 0xff),
        (0xff1f, 0xff),
        (0xff20, 0xff),
        (0xff21, 0x5a),
        (0xff22, 0x5a),
        (0xff23, 0xff),
        (0xff24, 0x5a),
        (0xff25, 0x5a),
    ];
    for (address, expected) in powered_on {
        let mut apu = test_apu();
        apu.write(address, 0x5a);
        assert_eq!(apu.read(address), expected, "powered-on {address:#06x}");
    }
    for address in 0xff27..=0xff2f {
        let mut apu = test_apu();
        apu.write(address, 0x5a);
        assert_eq!(apu.read(address), 0xff, "unused {address:#06x}");
    }
    for address in 0xff30..=0xff3f {
        let mut apu = test_apu();
        apu.write(address, 0x5a);
        assert_eq!(apu.read(address), 0x5a, "wave RAM {address:#06x}");
    }

    let powered_off = [
        (0xff10, 0x80),
        (0xff11, 0x3f),
        (0xff12, 0x00),
        (0xff13, 0xff),
        (0xff14, 0xbf),
        (0xff15, 0xff),
        (0xff16, 0x3f),
        (0xff17, 0x00),
        (0xff18, 0xff),
        (0xff19, 0xbf),
        (0xff1a, 0x7f),
        (0xff1b, 0xff),
        (0xff1c, 0x9f),
        (0xff1d, 0xff),
        (0xff1e, 0xbf),
        (0xff1f, 0xff),
        (0xff20, 0xff),
        (0xff21, 0x00),
        (0xff22, 0x00),
        (0xff23, 0xbf),
        (0xff24, 0x00),
        (0xff25, 0x00),
        (0xff26, 0x70),
    ];
    for (address, expected) in powered_off {
        let mut apu = test_apu();
        apu.write(0xff26, 0);
        apu.write(address, 0x5a);
        assert_eq!(apu.read(address), expected, "powered-off {address:#06x}");
    }
}

#[test]
fn frame_sequencer_uses_divider_bit_twelve_falling_edges() {
    let mut apu = test_apu();
    assert_eq!(apu.divider_mirror_for_test(), 0xabcc);
    for _ in 0..5_171 {
        apu.tick_t_cycle();
    }
    assert_eq!(apu.frame_step_for_test(), 0);
    apu.tick_t_cycle();
    assert_eq!(apu.frame_step_for_test(), 1);
    apu.reset();
    apu.write(0xff04, 0);
    assert_eq!(apu.frame_step_for_test(), 0);
    for _ in 0..8_192 {
        apu.tick_t_cycle();
    }
    assert_eq!(apu.frame_step_for_test(), 1);
    apu.reset();
    for _ in 0..1_076 {
        apu.tick_t_cycle();
    }
    assert_eq!(apu.divider_mirror_for_test(), 0xb000);
    apu.write(0xff04, 0);
    assert_eq!(apu.frame_step_for_test(), 1);
}

#[test]
fn length_counters_load_clock_and_apply_extra_clock() {
    let mut pulse = LengthCounter::<64>::default();
    pulse.load(63);
    assert_eq!(pulse.remaining(), 1);
    assert!(!pulse.set_enabled(true, true, true));
    assert!(pulse.clock());
    let mut wave = LengthCounter::<256>::default();
    wave.load(0);
    assert_eq!(wave.remaining(), 256);
    assert!(!wave.set_enabled(true, false, true));
    assert_eq!(wave.remaining(), 255);
    let mut reload = LengthCounter::<64>::default();
    let _ = reload.set_enabled(true, true, false);
    assert!(!reload.trigger(false));
    assert_eq!(reload.remaining(), 63);
}

#[test]
fn envelope_period_zero_clocks_every_eighth_step() {
    let mut envelope = VolumeEnvelope::from_register(0x58);
    envelope.trigger();
    for _ in 0..7 {
        envelope.clock();
    }
    assert_eq!(envelope.volume(), 5);
    envelope.clock();
    assert_eq!(envelope.volume(), 6);
}

#[test]
fn envelope_direction_and_saturation_matrix() {
    let mut decreasing = VolumeEnvelope::from_register(0x51);
    decreasing.trigger();
    assert_eq!(decreasing.volume(), 5);
    for expected in [4, 3, 2, 1, 0] {
        decreasing.clock();
        assert_eq!(decreasing.volume(), expected);
    }
    for _ in 0..4 {
        decreasing.clock();
        assert_eq!(decreasing.volume(), 0);
    }

    let mut increasing = VolumeEnvelope::from_register(0xe9);
    increasing.trigger();
    assert_eq!(increasing.volume(), 14);
    increasing.clock();
    assert_eq!(increasing.volume(), 15);
    for _ in 0..4 {
        increasing.clock();
        assert_eq!(increasing.volume(), 15);
    }

    increasing.write(0x39);
    increasing.trigger();
    assert_eq!(increasing.volume(), 3);
    increasing.clock();
    assert_eq!(increasing.volume(), 4);
}

#[test]
fn pulse_timer_advances_duty_and_dac_off_disables() {
    let mut pulse = PulseChannel::new(false);
    pulse.write(1, 0x80, true);
    pulse.write(2, 0xf0, true);
    pulse.write(3, 0xff, true);
    pulse.write(4, 0x87, true);
    assert!(pulse.active());
    for _ in 0..4 {
        pulse.tick_t_cycle();
    }
    assert_eq!(pulse.output(), 0);
    pulse.write(2, 0, true);
    assert!(!pulse.active());
}

#[test]
fn pulse_duty_and_frequency_period_matrix() {
    const DUTIES: [[u8; 8]; 4] = [
        [0, 0, 0, 0, 0, 0, 0, 1],
        [1, 0, 0, 0, 0, 0, 0, 1],
        [1, 0, 0, 0, 0, 1, 1, 1],
        [0, 1, 1, 1, 1, 1, 1, 0],
    ];

    for (duty, expected) in DUTIES.into_iter().enumerate() {
        let mut pulse = PulseChannel::new(false);
        pulse.write(
            1,
            u8::try_from(duty).expect("duty index fits in u8") << 6,
            true,
        );
        pulse.write(2, 0xf0, true);
        pulse.write(3, 0xff, true);
        pulse.write(4, 0x87, true);
        let mut actual = [0; 8];
        actual[0] = pulse.output();
        for sample in &mut actual[1..] {
            for _ in 0..4 {
                pulse.tick_t_cycle();
            }
            *sample = pulse.output();
        }
        assert_eq!(actual, expected.map(|bit| bit * 15), "duty {duty}");
    }

    for (frequency, period) in [(0_u16, 8_192_u16), (1_024, 4_096), (2_047, 4)] {
        let mut pulse = PulseChannel::new(false);
        pulse.write(1, 0x40, true);
        pulse.write(2, 0xf0, true);
        pulse.write(3, frequency.to_le_bytes()[0], true);
        pulse.write(
            4,
            u8::try_from(frequency >> 8).expect("pulse frequency high bits fit in u8") | 0x80,
            true,
        );
        assert_eq!(pulse.output(), 15, "frequency {frequency}");
        for _ in 0..period - 1 {
            pulse.tick_t_cycle();
        }
        assert_eq!(pulse.output(), 15, "before expiry at frequency {frequency}");
        pulse.tick_t_cycle();
        assert_eq!(pulse.output(), 0, "at expiry at frequency {frequency}");
    }
}

#[test]
fn sweep_applies_then_checks_the_next_overflow() {
    let mut sweep = FrequencySweep::default();
    assert!(!sweep.write(0x11));
    assert_eq!(sweep.trigger(0x500), SweepTrigger::Enabled);
    assert_eq!(sweep.clock(), SweepClock::AppliedAndDisabled(0x780));
}

#[test]
fn sweep_trigger_overflow_and_negate_clear_disable_channel() {
    let mut overflow = FrequencySweep::default();
    assert!(!overflow.write(0x11));
    assert_eq!(overflow.trigger(0x600), SweepTrigger::Disabled);

    let mut negate = FrequencySweep::default();
    assert!(!negate.write(0x19));
    assert_eq!(negate.trigger(0x100), SweepTrigger::Enabled);
    assert!(negate.write(0x11));

    let mut period_zero = FrequencySweep::default();
    assert!(!period_zero.write(0x01));
    assert_eq!(period_zero.trigger(0x200), SweepTrigger::Enabled);
    for _ in 0..7 {
        assert_eq!(period_zero.clock(), SweepClock::Idle);
    }
    assert_eq!(period_zero.clock(), SweepClock::Applied(0x300));

    let mut negate = FrequencySweep::default();
    assert!(!negate.write(0x19));
    assert_eq!(negate.trigger(0x100), SweepTrigger::Enabled);
    assert_eq!(negate.clock(), SweepClock::Applied(0x080));
    assert!(negate.write(0x11));
}

#[test]
fn wave_playback_advances_nibbles_and_scales_level() {
    let mut wave = WaveChannel::default();
    wave.write_wave_ram(0xff30, 0x12);
    wave.write_wave_ram(0xff31, 0x34);
    wave.write(0, 0x80, true);
    wave.write(2, 0x20, true);
    wave.write(3, 0xff, true);
    wave.write(4, 0x87, true);
    assert_eq!(wave.output(), 1);
    for _ in 0..2 {
        wave.tick_t_cycle();
    }
    assert_eq!(wave.output(), 2);
    for _ in 0..2 {
        wave.tick_t_cycle();
    }
    assert_eq!(wave.output(), 3);
}

#[test]
fn wave_output_level_matrix_and_active_ram_access_window() {
    for (level, expected) in [(0, 0), (1, 12), (2, 6), (3, 3)] {
        let mut wave = WaveChannel::default();
        wave.write_wave_ram(0xff30, 0xc0);
        wave.write(0, 0x80, true);
        wave.write(2, level << 5, true);
        wave.write(3, 0xff, true);
        wave.write(4, 0x87, true);
        assert_eq!(wave.output(), expected, "output level {level}");
    }

    let mut wave = WaveChannel::default();
    wave.write_wave_ram(0xff30, 0x12);
    wave.write_wave_ram(0xff3f, 0xfe);
    wave.write(0, 0x80, true);
    wave.write(2, 0x20, true);
    wave.write(3, 0xfe, true);
    wave.write(4, 0x87, true);
    assert_eq!(wave.read_wave_ram(0xff30), 0xff);
    wave.write_wave_ram(0xff30, 0x99);
    for _ in 0..4 {
        wave.tick_t_cycle();
    }
    assert_eq!(wave.read_wave_ram(0xff3f), 0x12);
    wave.write_wave_ram(0xff3f, 0xab);
    assert_eq!(wave.read_wave_ram(0xff30), 0xab);
    wave.tick_t_cycle();
    assert_eq!(wave.read_wave_ram(0xff3f), 0xab);
    wave.tick_t_cycle();
    assert_eq!(wave.read_wave_ram(0xff30), 0xff);
    wave.write_wave_ram(0xff3f, 0xcd);
    wave.write(0, 0, true);
    assert_eq!(wave.read_wave_ram(0xff30), 0xab);
    assert_eq!(wave.read_wave_ram(0xff3f), 0xfe);
}

#[test]
fn wave_retrigger_corrupts_current_byte_or_aligned_block() {
    const INITIAL: [u8; 16] = [
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f,
    ];

    let mut current_byte = WaveChannel::default();
    for (index, value) in INITIAL.into_iter().enumerate() {
        current_byte.write_wave_ram(
            0xff30 + u16::try_from(index).expect("wave RAM index fits in u16"),
            value,
        );
    }
    current_byte.write(0, 0x80, true);
    current_byte.write(2, 0x20, true);
    current_byte.write(3, 0xff, true);
    current_byte.write(4, 0x87, true);
    for _ in 0..8 {
        current_byte.tick_t_cycle();
    }
    current_byte.write(4, 0x87, true);
    current_byte.write(0, 0, true);
    assert_eq!(current_byte.read_wave_ram(0xff30), INITIAL[2]);

    let mut aligned_block = WaveChannel::default();
    for (index, value) in INITIAL.into_iter().enumerate() {
        aligned_block.write_wave_ram(
            0xff30 + u16::try_from(index).expect("wave RAM index fits in u16"),
            value,
        );
    }
    aligned_block.write(0, 0x80, true);
    aligned_block.write(2, 0x20, true);
    aligned_block.write(3, 0xff, true);
    aligned_block.write(4, 0x87, true);
    for _ in 0..16 {
        aligned_block.tick_t_cycle();
    }
    aligned_block.write(4, 0x87, true);
    aligned_block.write(0, 0, true);
    for (index, expected) in INITIAL[4..8].iter().copied().enumerate() {
        assert_eq!(
            aligned_block
                .read_wave_ram(0xff30 + u16::try_from(index).expect("wave RAM index fits in u16")),
            expected,
            "corrupted block byte {index}"
        );
    }
}

#[test]
fn noise_period_uses_nr43_divisor_and_shift() {
    for (code, divisor) in [
        (0, 8),
        (1, 16),
        (2, 32),
        (3, 48),
        (4, 64),
        (5, 80),
        (6, 96),
        (7, 112),
    ] {
        assert_eq!(noise_period(code, 0), divisor);
        assert_eq!(noise_period(code, 3), divisor << 3);
    }
    let mut noise = NoiseChannel::default();
    noise.write(1, 0xf0, true);
    noise.write(2, 0, true);
    noise.write(3, 0x80, true);
    assert!(noise.active());
    for _ in 0..8 {
        noise.tick_t_cycle();
    }
    assert!(noise.output() <= 15);
}

#[test]
fn noise_lfsr_sequences_distinguish_fifteen_and_seven_bit_modes() {
    for (width_register, first_high_step) in [(0x00, 15), (0x08, 7)] {
        let mut noise = NoiseChannel::default();
        noise.write(1, 0xf0, true);
        noise.write(2, width_register, true);
        noise.write(3, 0x80, true);
        assert_eq!(noise.output(), 0, "triggered width {width_register:#04x}");
        for step in 1..first_high_step {
            for _ in 0..8 {
                noise.tick_t_cycle();
            }
            assert_eq!(
                noise.output(),
                0,
                "width {width_register:#04x}, LFSR step {step}"
            );
        }
        for _ in 0..8 {
            noise.tick_t_cycle();
        }
        assert_eq!(
            noise.output(),
            15,
            "width {width_register:#04x}, first inverted-zero output"
        );
    }
}

#[test]
fn mixer_routes_each_channel_and_high_pass_removes_dc() {
    let mut mixer = Mixer::new(rate(48_000));
    let [left, right] = mixer.mix_with_dacs([15, 0, 0, 0], [true; 4], 0x77, 0x10);
    assert!(left > 0.0);
    assert!(right.abs() <= f32::EPSILON);
    let mut last = [0.0; 2];
    for _ in 0..48_000 {
        last = mixer.mix_with_dacs([15; 4], [true; 4], 0x77, 0xff);
    }
    assert!(last[0].abs() < 0.02);
    assert!(last[1].abs() < 0.02);
}

#[test]
fn mixer_routing_volume_vin_and_filter_reset_matrix() {
    for channel in 0..4 {
        let mut samples = [0; 4];
        samples[channel] = 15;
        let mut dacs = [false; 4];
        dacs[channel] = true;

        let mut left_mixer = Mixer::new(rate(48_000));
        assert_stereo_close(
            left_mixer.mix_with_dacs(samples, dacs, 0x77, 1 << (channel + 4)),
            [0.25, 0.0],
            &format!("left channel {}", channel + 1),
        );
        let mut right_mixer = Mixer::new(rate(48_000));
        assert_stereo_close(
            right_mixer.mix_with_dacs(samples, dacs, 0x77, 1 << channel),
            [0.0, 0.25],
            &format!("right channel {}", channel + 1),
        );
    }

    for volume in 0..=7_u8 {
        let nr50 = (volume << 4) | volume;
        let expected = 0.25 * f32::from(volume + 1) / 8.0;
        let mut mixer = Mixer::new(rate(48_000));
        let actual = mixer.mix_with_dacs([15, 0, 0, 0], [true, false, false, false], nr50, 0x11);
        assert_stereo_close(
            actual,
            [expected, expected],
            &format!("NR50 volume {volume}"),
        );

        let mut with_vin = Mixer::new(rate(48_000));
        assert_stereo_close(
            with_vin.mix_with_dacs(
                [15, 0, 0, 0],
                [true, false, false, false],
                nr50 | 0x88,
                0x11,
            ),
            actual,
            &format!("VIN bits at NR50 volume {volume}"),
        );
    }

    let mut mixer = Mixer::new(rate(48_000));
    let first = mixer.mix_with_dacs([0; 4], [true; 4], 0x77, 0xff);
    let second = mixer.mix_with_dacs([0; 4], [true; 4], 0x77, 0xff);
    assert!(
        first
            .into_iter()
            .zip(second)
            .any(|(first, second)| (first - second).abs() > f32::EPSILON)
    );
    mixer.reset();
    assert_stereo_close(
        mixer.mix_with_dacs([0; 4], [true; 4], 0x77, 0xff),
        first,
        "filter reset",
    );

    let mut silent = Mixer::new(rate(48_000));
    assert_stereo_close(
        silent.mix_with_dacs([15; 4], [false; 4], 0x77, 0xff),
        [0.0, 0.0],
        "disabled DAC silence",
    );
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn pcm_buffer_drops_oldest_complete_pair_and_drains() {
    let mut pcm = PcmBuffer::default();
    for frame in 0..=MAX_CORE_STEREO_FRAMES {
        pcm.push_stereo(frame as f32, -(frame as f32));
    }
    assert_eq!(pcm.stereo_frames_available(), MAX_CORE_STEREO_FRAMES);
    assert_eq!(pcm.dropped_stereo_frames(), 1);
    let batch = pcm.drain(rate(48_000));
    assert_eq!(batch.stereo_frame_count(), MAX_CORE_STEREO_FRAMES);
    assert!((batch.samples()[0] - 1.0).abs() <= f32::EPSILON);
    assert_eq!(pcm.drain(rate(48_000)).stereo_frame_count(), 0);
}

#[test]
fn power_off_and_reset_clear_pcm_without_changing_batch_rate() {
    let mut apu = test_apu();
    for _ in 0..100 {
        apu.tick_t_cycle();
    }
    assert!(apu.stereo_frames_available() > 0);
    apu.write(0xff26, 0);
    assert_eq!(apu.stereo_frames_available(), 0);
    assert_eq!(apu.drain_audio().sample_rate(), rate(48_000));
    for _ in 0..100 {
        apu.tick_t_cycle();
    }
    apu.reset();
    assert_eq!(apu.stereo_frames_available(), 0);
}

#[test]
fn one_dmg_second_generates_exactly_the_requested_rate_with_bounded_storage() {
    for hz in [44_100, 48_000, 96_000] {
        let mut apu = Apu::new(rate(hz));
        for _ in 0..DMG_CLOCK_HZ {
            apu.tick_t_cycle();
        }
        assert_eq!(apu.generated_stereo_frames_for_test(), u64::from(hz));
        assert_eq!(apu.stereo_frames_available(), MAX_CORE_STEREO_FRAMES);
        assert_eq!(apu.dropped_stereo_frames_for_test(), u64::from(hz) - 4_096);
        assert_eq!(apu.drain_audio().sample_rate(), rate(hz));
    }
}

#[test]
fn pcm_is_identical_across_uneven_tick_and_drain_partitions() {
    fn configured_tone() -> Apu {
        let mut apu = test_apu();
        apu.write(0xff24, 0x77);
        apu.write(0xff25, 0x22);
        apu.write(0xff16, 0x40);
        apu.write(0xff17, 0xf0);
        apu.write(0xff18, 0xff);
        apu.write(0xff19, 0x87);
        apu
    }

    fn render_chunks(apu: &mut Apu, chunks: &[usize]) -> Vec<f32> {
        let mut samples = Vec::new();
        for &t_cycles in chunks {
            for _ in 0..t_cycles {
                apu.tick_t_cycle();
            }
            let batch = apu.drain_audio();
            samples.extend_from_slice(batch.samples());
        }
        samples
    }

    let chunks = [1, 7, 8_191, 3, 65_537, 11, 104_729];
    let total = chunks.iter().sum();
    let continuous = render_chunks(&mut configured_tone(), &[total]);
    let partitioned = render_chunks(&mut configured_tone(), &chunks);
    assert!(!continuous.is_empty());
    assert_eq!(partitioned, continuous);
}
