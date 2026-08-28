use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use gb_core::AudioBatch;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use super::{AudioBackendError, AudioBackendErrorKind, AudioHealth, AudioWatermarks};

#[derive(Debug)]
struct QueueState {
    paused: AtomicBool,
    flush_epoch: AtomicU64,
    flushed_epoch: AtomicU64,
    underruns: AtomicU64,
    dropped_frames: AtomicU64,
    stream_errors: AtomicU64,
    usable: AtomicBool,
}

impl Default for QueueState {
    fn default() -> Self {
        Self {
            paused: AtomicBool::new(true),
            flush_epoch: AtomicU64::new(0),
            flushed_epoch: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            dropped_frames: AtomicU64::new(0),
            stream_errors: AtomicU64::new(0),
            usable: AtomicBool::new(true),
        }
    }
}

pub(crate) struct AudioQueueProducer {
    producer: HeapProd<f32>,
    sample_rate: NonZeroU32,
    state: Arc<QueueState>,
}

pub(crate) struct AudioQueueConsumer {
    consumer: HeapCons<f32>,
    state: Arc<QueueState>,
    observed_epoch: u64,
}

#[derive(Clone)]
pub(crate) struct AudioQueueErrorReporter {
    state: Arc<QueueState>,
}

pub(crate) fn new_queue(sample_rate: NonZeroU32) -> (AudioQueueProducer, AudioQueueConsumer) {
    let watermarks = AudioWatermarks::for_rate(sample_rate);
    let ring = HeapRb::<f32>::new(watermarks.sample_capacity());
    let (producer, consumer) = ring.split();
    let state = Arc::new(QueueState::default());
    (
        AudioQueueProducer {
            producer,
            sample_rate,
            state: Arc::clone(&state),
        },
        AudioQueueConsumer {
            consumer,
            state,
            observed_epoch: 0,
        },
    )
}

impl AudioQueueProducer {
    pub(crate) fn enqueue(&mut self, batch: &AudioBatch) -> Result<(), AudioBackendError> {
        if batch.sample_rate() != self.sample_rate {
            return Err(AudioBackendError::new(
                AudioBackendErrorKind::UnsupportedConfiguration,
                "audio batch sample rate mismatch",
            ));
        }
        let samples = batch.samples();
        if !samples.len().is_multiple_of(2) {
            return Err(AudioBackendError::new(
                AudioBackendErrorKind::Backend,
                "audio batch contains an incomplete stereo frame",
            ));
        }
        if self.flush_pending() {
            return Err(AudioBackendError::new(
                AudioBackendErrorKind::StreamInvalidated,
                "audio queue flush is pending",
            ));
        }
        if self.producer.vacant_len() < samples.len() {
            let dropped_frames = u64::try_from(samples.len() / 2)
                .expect("audio batch frame count fits the telemetry counter");
            self.state
                .dropped_frames
                .fetch_add(dropped_frames, Ordering::Relaxed);
            return Err(AudioBackendError::new(
                AudioBackendErrorKind::DeviceBusy,
                "audio queue capacity exceeded",
            ));
        }
        let pushed = self.producer.push_slice(samples);
        debug_assert_eq!(pushed, samples.len());
        Ok(())
    }

    pub(crate) fn health(&self) -> AudioHealth {
        AudioHealth {
            queued_stereo_frames: self.producer.occupied_len() / 2,
            flush_pending: self.flush_pending(),
            underruns: self.state.underruns.load(Ordering::Relaxed),
            dropped_stereo_frames: self.state.dropped_frames.load(Ordering::Relaxed),
            stream_errors: self.state.stream_errors.load(Ordering::Relaxed),
            usable: self.state.usable.load(Ordering::Acquire),
        }
    }

    pub(crate) fn pause_and_flush(&self) {
        self.state.paused.store(true, Ordering::Release);
        self.state.flush_epoch.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn play(&self) -> Result<(), AudioBackendError> {
        if self.flush_pending() {
            return Err(AudioBackendError::new(
                AudioBackendErrorKind::StreamInvalidated,
                "audio queue flush is pending",
            ));
        }
        self.state.paused.store(false, Ordering::Release);
        Ok(())
    }

    fn flush_pending(&self) -> bool {
        self.state.flush_epoch.load(Ordering::Acquire)
            != self.state.flushed_epoch.load(Ordering::Acquire)
    }
}

impl AudioQueueConsumer {
    pub(crate) fn error_reporter(&self) -> AudioQueueErrorReporter {
        AudioQueueErrorReporter {
            state: Arc::clone(&self.state),
        }
    }

    pub(crate) fn fill(&mut self, output: &mut [f32]) {
        fill_output(
            &mut self.consumer,
            output,
            &self.state.paused,
            &self.state.flush_epoch,
            &self.state.flushed_epoch,
            &mut self.observed_epoch,
            &self.state.underruns,
        );
    }
}

impl AudioQueueErrorReporter {
    pub(crate) fn report(&self, kind: cpal::ErrorKind) {
        if kind == cpal::ErrorKind::Xrun {
            self.state.underruns.fetch_add(1, Ordering::Relaxed);
        } else {
            self.state.stream_errors.fetch_add(1, Ordering::Relaxed);
        }
        if matches!(
            kind,
            cpal::ErrorKind::DeviceNotAvailable | cpal::ErrorKind::StreamInvalidated
        ) {
            self.state.usable.store(false, Ordering::Release);
        }
    }
}

pub(crate) fn fill_output(
    consumer: &mut HeapCons<f32>,
    output: &mut [f32],
    paused: &AtomicBool,
    flush_epoch: &AtomicU64,
    flushed_epoch: &AtomicU64,
    observed_epoch: &mut u64,
    underruns: &AtomicU64,
) {
    debug_assert!(output.len().is_multiple_of(2));
    let epoch = flush_epoch.load(Ordering::Acquire);
    if epoch != *observed_epoch {
        consumer.clear();
        *observed_epoch = epoch;
        flushed_epoch.store(epoch, Ordering::Release);
    }
    if paused.load(Ordering::Acquire) {
        output.fill(0.0);
        return;
    }
    let copied = consumer.pop_slice(output);
    output[copied..].fill(0.0);
    if copied < output.len() {
        underruns.fetch_add(1, Ordering::Relaxed);
    }
    if !output.len().is_multiple_of(2) {
        output[output.len() - 1] = 0.0;
    }
}
