use std::num::NonZeroU32;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, ErrorKind, SampleFormat, StreamConfig, SupportedBufferSize, SupportedStreamConfig,
};
use gb_core::AudioBatch;

use super::queue::{AudioQueueProducer, new_queue};
use super::{
    AudioBackendError, AudioBackendErrorKind, AudioHealth, AudioOutput, AudioOutputFactory,
    AudioWatermarks,
};

pub(crate) struct CpalAudioOutputFactory;

pub(crate) struct CpalAudioOutput {
    stream: Option<cpal::Stream>,
    producer: AudioQueueProducer,
    sample_rate: NonZeroU32,
    watermarks: AudioWatermarks,
}

impl AudioOutputFactory for CpalAudioOutputFactory {
    fn open_default(&self) -> Result<Box<dyn AudioOutput>, AudioBackendError> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or_else(|| {
            AudioBackendError::new(
                AudioBackendErrorKind::NoOutputDevice,
                "no default audio output is available",
            )
        })?;
        let ranges = device
            .supported_output_configs()
            .map_err(|error| map_error(&error))?;
        let supported = select_supported_config(ranges.collect())?;
        let sample_rate = NonZeroU32::new(supported.sample_rate()).ok_or_else(|| {
            AudioBackendError::new(
                AudioBackendErrorKind::UnsupportedConfiguration,
                "audio device reported a zero sample rate",
            )
        })?;
        let buffer_size = *supported.buffer_size();
        let mut config: StreamConfig = supported.into();
        apply_supported_buffer_size(&mut config, &buffer_size);
        let watermarks = AudioWatermarks::for_rate(sample_rate);
        let (producer, mut consumer) = new_queue(sample_rate);
        let reporter = consumer.error_reporter();
        let stream = device
            .build_output_stream::<f32, _, _>(
                config,
                move |output, _| consumer.fill(output),
                move |error| reporter.report(error.kind()),
                Some(Duration::from_secs(2)),
            )
            .map_err(|error| map_error(&error))?;
        stream.play().map_err(|error| map_error(&error))?;
        Ok(Box::new(CpalAudioOutput {
            stream: Some(stream),
            producer,
            sample_rate,
            watermarks,
        }))
    }
}

impl AudioOutput for CpalAudioOutput {
    fn sample_rate(&self) -> NonZeroU32 {
        self.sample_rate
    }
    fn watermarks(&self) -> AudioWatermarks {
        self.watermarks
    }
    fn enqueue(&mut self, batch: &AudioBatch) -> Result<(), AudioBackendError> {
        self.producer.enqueue(batch)
    }
    fn set_gain(&mut self, gain: f32) -> Result<(), AudioBackendError> {
        self.producer.set_gain(gain)
    }
    fn health(&self) -> AudioHealth {
        self.producer.health()
    }
    fn play(&mut self) -> Result<(), AudioBackendError> {
        self.producer.play()
    }
    fn pause_and_flush(&mut self) -> Result<(), AudioBackendError> {
        self.producer.pause_and_flush();
        Ok(())
    }
    fn shutdown(&mut self) {
        self.producer.pause_and_flush();
        if let Some(stream) = self.stream.take() {
            let _ = stream.pause();
            drop(stream);
        }
    }
}

impl Drop for CpalAudioOutput {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn select_supported_config(
    ranges: Vec<cpal::SupportedStreamConfigRange>,
) -> Result<SupportedStreamConfig, AudioBackendError> {
    let eligible: Vec<_> = ranges
        .into_iter()
        .filter(|range| range.sample_format() == SampleFormat::F32 && range.channels() == 2)
        .collect();
    for range in &eligible {
        if let Some(config) = (*range).try_with_sample_rate(48_000) {
            return Ok(config);
        }
    }
    eligible
        .into_iter()
        .filter_map(cpal::SupportedStreamConfigRange::try_with_standard_sample_rate)
        .min_by_key(|config| (config.sample_rate().abs_diff(48_000), config.sample_rate()))
        .ok_or_else(|| {
            AudioBackendError::new(
                AudioBackendErrorKind::UnsupportedConfiguration,
                "default output has no F32 stereo standard-rate configuration",
            )
        })
}

fn apply_supported_buffer_size(config: &mut StreamConfig, supported: &SupportedBufferSize) {
    config.buffer_size = match *supported {
        SupportedBufferSize::Range { min, max } if min <= 512 && 512 <= max => {
            BufferSize::Fixed(512)
        }
        _ => BufferSize::Default,
    };
}

pub(crate) const fn map_error_kind(kind: ErrorKind) -> AudioBackendErrorKind {
    match kind {
        ErrorKind::DeviceNotAvailable | ErrorKind::HostUnavailable => {
            AudioBackendErrorKind::DeviceUnavailable
        }
        ErrorKind::DeviceBusy => AudioBackendErrorKind::DeviceBusy,
        ErrorKind::PermissionDenied => AudioBackendErrorKind::PermissionDenied,
        ErrorKind::UnsupportedConfig | ErrorKind::UnsupportedOperation => {
            AudioBackendErrorKind::UnsupportedConfiguration
        }
        ErrorKind::StreamInvalidated => AudioBackendErrorKind::StreamInvalidated,
        _ => AudioBackendErrorKind::Backend,
    }
}

fn map_error(error: &cpal::Error) -> AudioBackendError {
    AudioBackendError::new(map_error_kind(error.kind()), error.kind().to_string())
}

#[cfg(test)]
pub(crate) fn configure_for_test(
    range: cpal::SupportedStreamConfigRange,
) -> Result<StreamConfig, AudioBackendError> {
    let supported = select_supported_config(vec![range])?;
    let buffer = *supported.buffer_size();
    let mut config = StreamConfig::from(supported);
    apply_supported_buffer_size(&mut config, &buffer);
    Ok(config)
}

#[cfg(test)]
pub(crate) fn select_configs_for_test(
    ranges: Vec<cpal::SupportedStreamConfigRange>,
) -> Result<SupportedStreamConfig, AudioBackendError> {
    select_supported_config(ranges)
}
