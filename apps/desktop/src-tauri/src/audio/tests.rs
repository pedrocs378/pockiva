use std::num::NonZeroU32;
use std::time::Duration;

use cpal::{BufferSize, ErrorKind, SampleFormat, SupportedBufferSize, SupportedStreamConfigRange};
use gb_core::AudioBatch;

use super::cpal_backend::{configure_for_test, map_error_kind, select_configs_for_test};
use super::queue::new_queue;
use super::*;

fn rate(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).expect("non-zero test rate")
}

mod policy {
    use super::*;

    #[test]
    fn forty_to_one_sixty_ms_watermarks_scale_with_rate() {
        for (hz, expected) in [
            (48_000, [1_920, 3_840, 5_760, 7_680]),
            (44_100, [1_764, 3_528, 5_292, 7_056]),
            (96_000, [3_840, 7_680, 11_520, 15_360]),
            (44_117, [1_764, 3_529, 5_294, 7_058]),
        ] {
            let marks = AudioWatermarks::for_rate(rate(hz));
            assert_eq!(marks.sample_rate, rate(hz));
            assert_eq!(
                [marks.low, marks.target, marks.high, marks.capacity],
                expected
            );
            assert_eq!(marks.sample_capacity(), expected[3] * 2);
        }
    }

    fn health(queued_stereo_frames: usize, usable: bool) -> AudioHealth {
        AudioHealth {
            queued_stereo_frames,
            flush_pending: false,
            underruns: 0,
            dropped_stereo_frames: 0,
            stream_errors: 0,
            usable,
        }
    }

    #[test]
    fn pacing_uses_all_boundaries_and_exact_rate_duration() {
        let marks = AudioWatermarks::for_rate(rate(44_117));
        assert_eq!(
            pacing_decision(health(marks.low - 1, true), marks),
            PacingDecision::Prime
        );
        assert_eq!(
            pacing_decision(health(marks.low, true), marks),
            PacingDecision::RunOneBatch
        );
        assert_eq!(
            pacing_decision(health(marks.target, true), marks),
            PacingDecision::RunOneBatch
        );
        assert_eq!(
            pacing_decision(health(marks.target + 100, true), marks),
            PacingDecision::Wait(Duration::from_nanos(2_266_700)),
        );
        assert_eq!(
            pacing_decision(health(marks.high + 1, true), marks),
            PacingDecision::Backpressured(Duration::from_millis(5)),
        );
        assert_eq!(
            pacing_decision(health(0, false), marks),
            PacingDecision::FallbackFrameDeadline
        );
    }
}

mod queue {
    use super::*;

    #[allow(clippy::cast_precision_loss)]
    fn batch(rate: NonZeroU32, frames: usize) -> AudioBatch {
        let samples = (0..frames)
            .flat_map(|frame| [frame as f32, -(frame as f32)])
            .collect();
        AudioBatch::new(rate, samples).expect("valid test batch")
    }

    #[test]
    fn enqueue_is_all_or_nothing_and_tracks_dropped_frames() {
        let (mut producer, _consumer) = new_queue(rate(48_000));
        producer
            .enqueue(&batch(rate(48_000), 7_680))
            .expect("fill capacity");
        let error = producer
            .enqueue(&batch(rate(48_000), 2))
            .expect_err("overflow rejected");
        assert_eq!(error.kind, AudioBackendErrorKind::DeviceBusy);
        let health = producer.health();
        assert_eq!(health.queued_stereo_frames, 7_680);
        assert_eq!(health.dropped_stereo_frames, 2);
        let mismatch = producer
            .enqueue(&batch(rate(44_100), 1))
            .expect_err("rate mismatch rejected");
        assert_eq!(
            mismatch.kind,
            AudioBackendErrorKind::UnsupportedConfiguration
        );
    }

    #[test]
    fn callback_copies_silences_underrun_and_acknowledges_flush() {
        let (mut producer, mut consumer) = new_queue(rate(48_000));
        producer
            .enqueue(&AudioBatch::new(rate(48_000), vec![1.0, -1.0, 0.5, -0.5]).unwrap())
            .unwrap();
        producer.play().unwrap();
        let mut output = [9.0; 6];
        consumer.fill(&mut output);
        assert!(
            output
                .into_iter()
                .zip([1.0, -1.0, 0.5, -0.5, 0.0, 0.0])
                .all(|(actual, expected)| (actual - expected).abs() <= f32::EPSILON)
        );
        assert_eq!(producer.health().underruns, 1);
        producer.enqueue(&batch(rate(48_000), 2)).unwrap();
        producer.pause_and_flush();
        assert!(producer.health().flush_pending);
        output.fill(9.0);
        consumer.fill(&mut output);
        assert!(
            output
                .into_iter()
                .all(|sample| sample.abs() <= f32::EPSILON)
        );
        assert!(!producer.health().flush_pending);
        assert_eq!(producer.health().queued_stereo_frames, 0);
    }

    #[test]
    fn callback_applies_gain_and_supports_muting_without_changing_queue_depth() {
        let (mut producer, mut consumer) = new_queue(rate(48_000));
        producer
            .enqueue(&AudioBatch::new(rate(48_000), vec![1.0, -1.0, 0.5, -0.5]).unwrap())
            .unwrap();
        producer.set_gain(0.25).unwrap();
        producer.play().unwrap();

        let mut output = [9.0; 4];
        consumer.fill(&mut output);
        assert!(
            output
                .into_iter()
                .zip([0.25, -0.25, 0.125, -0.125])
                .all(|(actual, expected)| (actual - expected).abs() <= f32::EPSILON)
        );

        producer
            .enqueue(&AudioBatch::new(rate(48_000), vec![1.0, -1.0]).unwrap())
            .unwrap();
        producer.set_gain(0.0).unwrap();
        let queued_before_mute = producer.health().queued_stereo_frames;
        consumer.fill(&mut output[..2]);
        assert_eq!(queued_before_mute, 1);
        assert_eq!(&output[..2], &[0.0, 0.0]);
        assert_eq!(producer.health().queued_stereo_frames, 0);
    }

    #[test]
    fn gain_rejects_non_finite_and_out_of_range_values() {
        let (producer, _consumer) = new_queue(rate(48_000));
        for gain in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
            assert_eq!(
                producer.set_gain(gain).unwrap_err().kind,
                AudioBackendErrorKind::Backend
            );
        }
    }

    #[test]
    fn callback_error_health_distinguishes_xruns_from_terminal_errors() {
        let (producer, consumer) = new_queue(rate(48_000));
        let reporter = consumer.error_reporter();
        reporter.report(ErrorKind::Xrun);
        assert_eq!(producer.health().underruns, 1);
        assert!(producer.health().usable);
        reporter.report(ErrorKind::StreamInvalidated);
        assert_eq!(producer.health().stream_errors, 1);
        assert!(!producer.health().usable);
    }
}

mod backend {
    use super::*;

    #[test]
    fn explicit_f32_stereo_config_and_fixed_buffer_are_selected() {
        let range = SupportedStreamConfigRange::new(
            2,
            44_100,
            96_000,
            SupportedBufferSize::Range {
                min: 128,
                max: 1_024,
            },
            SampleFormat::F32,
        );
        let config = configure_for_test(range).expect("eligible config");
        assert_eq!(config.channels, 2);
        assert_eq!(config.sample_rate, 48_000);
        assert_eq!(config.buffer_size, BufferSize::Fixed(512));
    }

    #[test]
    fn exact_forty_eight_kilohertz_wins_across_ranges() {
        let standard_only = SupportedStreamConfigRange::new(
            2,
            44_100,
            44_100,
            SupportedBufferSize::Unknown,
            SampleFormat::F32,
        );
        let exact = SupportedStreamConfigRange::new(
            2,
            48_000,
            96_000,
            SupportedBufferSize::Range { min: 600, max: 900 },
            SampleFormat::F32,
        );
        let selected =
            select_configs_for_test(vec![standard_only, exact]).expect("eligible config");
        assert_eq!(selected.sample_rate(), 48_000);
        let config = configure_for_test(exact).expect("eligible config");
        assert_eq!(config.buffer_size, BufferSize::Default);
    }

    #[test]
    fn unsupported_formats_and_error_kinds_are_typed() {
        let range = SupportedStreamConfigRange::new(
            1,
            44_100,
            48_000,
            SupportedBufferSize::Unknown,
            SampleFormat::I16,
        );
        assert_eq!(
            configure_for_test(range).unwrap_err().kind,
            AudioBackendErrorKind::UnsupportedConfiguration
        );
        for (source, expected) in [
            (
                ErrorKind::DeviceNotAvailable,
                AudioBackendErrorKind::DeviceUnavailable,
            ),
            (ErrorKind::DeviceBusy, AudioBackendErrorKind::DeviceBusy),
            (
                ErrorKind::PermissionDenied,
                AudioBackendErrorKind::PermissionDenied,
            ),
            (
                ErrorKind::UnsupportedConfig,
                AudioBackendErrorKind::UnsupportedConfiguration,
            ),
            (
                ErrorKind::UnsupportedOperation,
                AudioBackendErrorKind::UnsupportedConfiguration,
            ),
            (
                ErrorKind::StreamInvalidated,
                AudioBackendErrorKind::StreamInvalidated,
            ),
            (ErrorKind::Other, AudioBackendErrorKind::Backend),
        ] {
            assert_eq!(map_error_kind(source), expected);
        }
    }
}

#[test]
#[ignore = "optional developer-only CPAL tone; never a PED-49 gate"]
#[allow(clippy::cast_precision_loss)]
fn cpal_synthetic_tone_smoke() {
    let factory = CpalAudioOutputFactory;
    let mut output = factory.open_default().expect("default audio output");
    let rate = output.sample_rate();
    let frames = usize::try_from((u64::from(rate.get()) * 80) / 1_000).expect("80 ms frame count");
    let mut samples = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        let phase = std::f32::consts::TAU * 440.0 * frame as f32 / rate.get() as f32;
        let sample = phase.sin() * 0.10;
        samples.extend_from_slice(&[sample, sample]);
    }
    output
        .enqueue(&AudioBatch::new(rate, samples).expect("valid stereo tone"))
        .expect("enqueue tone");
    output.play().expect("play tone");
    std::thread::sleep(Duration::from_millis(100));
    output.shutdown();
}
