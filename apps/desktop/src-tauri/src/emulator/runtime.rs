use std::fs;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use gb_core::{
    AudioBatch, Button, CartridgeMetadata, EmulatorCore, Frame, InputSourceId, JoypadState,
};

use super::contracts::{
    RomSummary, RuntimeButton, RuntimeError, RuntimeErrorCode, RuntimeEvent, RuntimePhase,
    RuntimeResult, RuntimeSnapshot,
};
use crate::audio::{
    AudioBackendError, AudioOutput, AudioOutputFactory, PacingDecision, pacing_decision,
};
use crate::video::{AcknowledgeError, FrameQueue, encode_frame_packet};

const COMMAND_QUEUE_CAPACITY: usize = 32;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
const FRAME_INTERVAL: Duration = Duration::from_micros(16_743);
const FRAME_CYCLE_BUDGET: u32 = 70_224;
const AUDIO_POLL_INTERVAL: Duration = Duration::from_millis(5);
const MAX_PRIME_BATCHES: usize = 4;
const KEYBOARD_INPUT_SOURCE: InputSourceId = InputSourceId::new(1);

pub trait RuntimeCore: EmulatorCore + Send {}
impl<T: EmulatorCore + Send> RuntimeCore for T {}

pub trait CoreFactory: Send + Sync {
    fn create(&self) -> Box<dyn RuntimeCore>;
}

pub trait RuntimeObserver: Send + Sync {
    fn publish_control(&self, event: RuntimeEvent) -> RuntimeResult<()>;
    fn publish_frame(&self, packet: Vec<u8>) -> RuntimeResult<()>;
}

#[derive(Default)]
struct RuntimeModel {
    snapshot: RuntimeSnapshot,
    core_loaded: bool,
}

impl RuntimeModel {
    fn snapshot(&self) -> RuntimeSnapshot {
        self.snapshot.clone()
    }

    fn begin_load(&mut self) {
        self.core_loaded = false;
        self.snapshot = RuntimeSnapshot {
            phase: RuntimePhase::Loading,
            rom: None,
            error: None,
        };
    }

    fn finish_load(&mut self, metadata: CartridgeMetadata, file_name: String) {
        self.core_loaded = true;
        self.snapshot = RuntimeSnapshot {
            phase: RuntimePhase::Paused,
            rom: Some(RomSummary::from_metadata(metadata, file_name)),
            error: None,
        };
    }

    fn fail_load(&mut self, error: RuntimeError) {
        self.core_loaded = false;
        self.snapshot = RuntimeSnapshot {
            phase: RuntimePhase::Error,
            rom: None,
            error: Some(error),
        };
    }

    fn start(&mut self) -> RuntimeResult<()> {
        self.require_loaded()?;
        self.snapshot.phase = RuntimePhase::Running;
        Ok(())
    }

    fn pause(&mut self) -> RuntimeResult<()> {
        self.require_loaded()?;
        self.snapshot.phase = RuntimePhase::Paused;
        Ok(())
    }

    fn restart(&mut self) -> RuntimeResult<()> {
        self.require_loaded()?;
        self.snapshot.phase = RuntimePhase::Running;
        Ok(())
    }

    fn close(&mut self) {
        self.core_loaded = false;
        self.snapshot = RuntimeSnapshot::empty();
    }

    fn require_loaded(&self) -> RuntimeResult<()> {
        if self.core_loaded {
            Ok(())
        } else {
            Err(RuntimeError::new(
                RuntimeErrorCode::InvalidLifecycle,
                "No ROM is loaded.",
            ))
        }
    }
}

enum RuntimeCommand {
    Subscribe {
        observer: Arc<dyn RuntimeObserver>,
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    AcknowledgeFrame {
        sequence: u64,
        reply: SyncSender<RuntimeResult<()>>,
    },
    OpenRom {
        path: PathBuf,
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    Start {
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    Pause {
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    Restart {
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    Close {
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    SetKeyboardInput {
        buttons: Vec<RuntimeButton>,
        reply: SyncSender<RuntimeResult<()>>,
    },
    Snapshot {
        reply: SyncSender<RuntimeResult<RuntimeSnapshot>>,
    },
    #[cfg(test)]
    RunTicks {
        count: usize,
        reply: SyncSender<RuntimeResult<()>>,
    },
    #[cfg(test)]
    BufferedFrameCount {
        reply: SyncSender<RuntimeResult<usize>>,
    },
    Shutdown {
        reply: SyncSender<RuntimeResult<()>>,
    },
}

struct RuntimeAudio {
    factory: Arc<dyn AudioOutputFactory>,
    sample_rate: NonZeroU32,
    prepared_output: Option<Box<dyn AudioOutput>>,
    first_open: bool,
    output: Option<Box<dyn AudioOutput>>,
    playing: bool,
    disabled_reason: Option<AudioBackendError>,
}

impl RuntimeAudio {
    fn new(
        factory: Arc<dyn AudioOutputFactory>,
        sample_rate: NonZeroU32,
        prepared_output: Option<Box<dyn AudioOutput>>,
        disabled_reason: Option<AudioBackendError>,
    ) -> Self {
        Self {
            factory,
            sample_rate,
            prepared_output,
            first_open: true,
            output: None,
            playing: false,
            disabled_reason,
        }
    }

    fn prepare_candidate(&mut self) -> Option<Box<dyn AudioOutput>> {
        let candidate = if self.first_open {
            self.first_open = false;
            let output = self.prepared_output.take()?;
            Ok(output)
        } else {
            self.factory.open_default()
        };
        match candidate {
            Ok(output) if output.sample_rate() == self.sample_rate => {
                let _ = self.disabled_reason.take();
                Some(output)
            }
            Ok(mut output) => {
                let actual = output.sample_rate();
                output.shutdown();
                self.disabled_reason = Some(AudioBackendError::new(
                    crate::audio::AudioBackendErrorKind::UnsupportedConfiguration,
                    format!(
                        "audio output rate {} does not match runtime rate {}",
                        actual, self.sample_rate
                    ),
                ));
                None
            }
            Err(error) => {
                self.disabled_reason = Some(error);
                None
            }
        }
    }

    fn activate(&mut self, output: Option<Box<dyn AudioOutput>>) {
        self.output = output;
        self.playing = false;
    }

    fn pause(&mut self) {
        if let Some(output) = self.output.as_deref_mut()
            && let Err(error) = output.pause_and_flush()
        {
            self.disable(error);
            return;
        }
        self.playing = false;
    }

    fn shutdown_output(&mut self) {
        if let Some(mut output) = self.output.take() {
            let _ = output.pause_and_flush();
            output.shutdown();
        }
        self.playing = false;
    }

    fn shutdown_all(&mut self) {
        self.shutdown_output();
        if let Some(mut output) = self.prepared_output.take() {
            let _ = output.pause_and_flush();
            output.shutdown();
        }
    }

    fn disable(&mut self, error: AudioBackendError) {
        self.shutdown_output();
        self.disabled_reason = Some(error);
    }

    fn disable_if_unusable(&mut self) {
        let unusable = self
            .output
            .as_deref()
            .is_some_and(|output| !output.health().usable);
        if unusable {
            self.disable(AudioBackendError::new(
                crate::audio::AudioBackendErrorKind::StreamInvalidated,
                "audio output became unusable",
            ));
        }
    }

    fn enqueue(&mut self, batch: &AudioBatch) {
        if batch.sample_rate() != self.sample_rate {
            let error = AudioBackendError::new(
                crate::audio::AudioBackendErrorKind::UnsupportedConfiguration,
                format!(
                    "core audio rate {} does not match runtime rate {}",
                    batch.sample_rate(),
                    self.sample_rate
                ),
            );
            if self.output.is_some() {
                self.disable(error);
            } else {
                self.disabled_reason = Some(error);
            }
            return;
        }
        let Some(output) = self.output.as_deref_mut() else {
            return;
        };
        if batch.stereo_frame_count() > 0
            && let Err(error) = output.enqueue(batch)
        {
            self.disable(error);
        }
    }

    fn start_playback_if_primed(&mut self) {
        let should_play = self.output.as_deref().is_some_and(|output| {
            let health = output.health();
            !health.flush_pending && health.queued_stereo_frames >= output.watermarks().target
        });
        if should_play
            && !self.playing
            && let Some(output) = self.output.as_deref_mut()
        {
            match output.play() {
                Ok(()) => self.playing = true,
                Err(error) => self.disable(error),
            }
        }
    }
}

pub struct DesktopRuntime {
    sender: SyncSender<RuntimeCommand>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

#[cfg(test)]
struct UnavailableAudioFactory;

#[cfg(test)]
impl AudioOutputFactory for UnavailableAudioFactory {
    fn open_default(&self) -> Result<Box<dyn AudioOutput>, AudioBackendError> {
        Err(AudioBackendError::new(
            crate::audio::AudioBackendErrorKind::NoOutputDevice,
            "audio is disabled for legacy runtime tests",
        ))
    }
}

impl DesktopRuntime {
    #[cfg(test)]
    #[must_use]
    pub fn spawn(factory: Arc<dyn CoreFactory>) -> Self {
        Self::spawn_with_audio(
            factory,
            Arc::new(UnavailableAudioFactory),
            NonZeroU32::new(48_000).expect("48 kHz is non-zero"),
            None,
        )
    }

    #[cfg(test)]
    #[must_use]
    pub fn spawn_with_audio(
        factory: Arc<dyn CoreFactory>,
        audio_factory: Arc<dyn AudioOutputFactory>,
        runtime_sample_rate: NonZeroU32,
        prepared_output: Option<Box<dyn AudioOutput>>,
    ) -> Self {
        let prepared_error = prepared_output.is_none().then(|| {
            AudioBackendError::new(
                crate::audio::AudioBackendErrorKind::NoOutputDevice,
                "audio output was unavailable during startup preflight",
            )
        });
        Self::spawn_audio_worker(
            factory,
            audio_factory,
            runtime_sample_rate,
            prepared_output,
            prepared_error,
        )
    }

    #[must_use]
    pub(crate) fn spawn_with_audio_preflight(
        factory: Arc<dyn CoreFactory>,
        audio_factory: Arc<dyn AudioOutputFactory>,
        runtime_sample_rate: NonZeroU32,
        prepared_output: Result<Box<dyn AudioOutput>, AudioBackendError>,
    ) -> Self {
        let (prepared_output, prepared_error) = match prepared_output {
            Ok(output) => (Some(output), None),
            Err(error) => (None, Some(error)),
        };
        Self::spawn_audio_worker(
            factory,
            audio_factory,
            runtime_sample_rate,
            prepared_output,
            prepared_error,
        )
    }

    fn spawn_audio_worker(
        factory: Arc<dyn CoreFactory>,
        audio_factory: Arc<dyn AudioOutputFactory>,
        runtime_sample_rate: NonZeroU32,
        prepared_output: Option<Box<dyn AudioOutput>>,
        prepared_error: Option<AudioBackendError>,
    ) -> Self {
        let (sender, receiver) = mpsc::sync_channel(COMMAND_QUEUE_CAPACITY);
        let worker = thread::Builder::new()
            .name("gameboy-desktop-runtime".into())
            .spawn(move || {
                run_worker(
                    &receiver,
                    &factory,
                    audio_factory,
                    runtime_sample_rate,
                    prepared_output,
                    prepared_error,
                );
            })
            .expect("desktop runtime worker starts");
        Self {
            sender,
            worker: Mutex::new(Some(worker)),
        }
    }

    pub fn snapshot(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Snapshot { reply })
    }

    pub fn subscribe(&self, observer: Arc<dyn RuntimeObserver>) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Subscribe { observer, reply })
    }

    pub fn acknowledge_frame(&self, sequence: u64) -> RuntimeResult<()> {
        self.request(|reply| RuntimeCommand::AcknowledgeFrame { sequence, reply })
    }

    pub fn open_rom(&self, path: PathBuf) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::OpenRom { path, reply })
    }

    pub fn start(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Start { reply })
    }

    pub fn pause(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Pause { reply })
    }

    pub fn restart(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Restart { reply })
    }

    pub fn close(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.request(|reply| RuntimeCommand::Close { reply })
    }

    pub fn set_keyboard_input(&self, buttons: Vec<RuntimeButton>) -> RuntimeResult<()> {
        self.request(|reply| RuntimeCommand::SetKeyboardInput { buttons, reply })
    }

    #[cfg(test)]
    fn test_only_run_ticks(&self, count: usize) -> RuntimeResult<()> {
        self.request(|reply| RuntimeCommand::RunTicks { count, reply })
    }

    #[cfg(test)]
    fn test_only_buffered_frame_count(&self) -> usize {
        self.request(|reply| RuntimeCommand::BufferedFrameCount { reply })
            .expect("runtime reports buffered frame count")
    }

    #[cfg(test)]
    fn test_only_fill_command_queue(&self) {
        for _ in 0..COMMAND_QUEUE_CAPACITY {
            let (reply, _response) = mpsc::sync_channel(1);
            self.sender
                .try_send(RuntimeCommand::Snapshot { reply })
                .expect("blocked worker leaves room for the exact bounded queue");
        }
    }

    pub fn shutdown(&self) -> RuntimeResult<()> {
        let mut worker = self.worker.lock().map_err(|_| runtime_unavailable())?;
        let Some(handle) = worker.take() else {
            return Ok(());
        };
        let (reply, response) = mpsc::sync_channel(1);
        let response = if self.sender.send(RuntimeCommand::Shutdown { reply }).is_ok() {
            match response.recv_timeout(RESPONSE_TIMEOUT) {
                Ok(response) => response,
                Err(_) => Err(runtime_unavailable()),
            }
        } else {
            Err(runtime_unavailable())
        };
        handle.join().map_err(|_| runtime_unavailable())?;
        response
    }

    fn request<T>(
        &self,
        command: impl FnOnce(SyncSender<RuntimeResult<T>>) -> RuntimeCommand,
    ) -> RuntimeResult<T> {
        let (reply, response) = mpsc::sync_channel(1);
        self.sender
            .try_send(command(reply))
            .map_err(|error| match error {
                TrySendError::Full(_) | TrySendError::Disconnected(_) => runtime_unavailable(),
            })?;
        response
            .recv_timeout(RESPONSE_TIMEOUT)
            .map_err(|_| runtime_unavailable())?
    }
}

impl Drop for DesktopRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn run_worker(
    receiver: &Receiver<RuntimeCommand>,
    factory: &Arc<dyn CoreFactory>,
    audio_factory: Arc<dyn AudioOutputFactory>,
    runtime_sample_rate: NonZeroU32,
    prepared_output: Option<Box<dyn AudioOutput>>,
    prepared_error: Option<AudioBackendError>,
) {
    let mut model = RuntimeModel::default();
    let mut core: Option<Box<dyn RuntimeCore>> = None;
    let mut observer: Option<Arc<dyn RuntimeObserver>> = None;
    let mut delivery = FrameQueue::default();
    let mut audio = RuntimeAudio::new(
        audio_factory,
        runtime_sample_rate,
        prepared_output,
        prepared_error,
    );
    let mut next_frame_deadline: Option<Instant> = None;

    loop {
        audio.disable_if_unusable();
        let now = Instant::now();
        let timeout = if model.snapshot.phase != RuntimePhase::Running {
            next_frame_deadline = None;
            IDLE_POLL_INTERVAL
        } else if audio.output.is_some() {
            next_frame_deadline = None;
            pump_audio_runtime(
                &mut model,
                core.as_deref_mut(),
                &mut observer,
                &mut delivery,
                &mut audio,
            )
        } else {
            let deadline = *next_frame_deadline.get_or_insert(now + FRAME_INTERVAL);
            if now >= deadline {
                run_tick(
                    &mut model,
                    core.as_deref_mut(),
                    &mut observer,
                    &mut delivery,
                    &mut audio,
                );
                next_frame_deadline = (model.snapshot.phase == RuntimePhase::Running)
                    .then(|| advance_frame_deadline(deadline, Instant::now()));
                continue;
            }
            deadline.saturating_duration_since(now)
        };
        match receiver.recv_timeout(timeout) {
            Ok(command) => {
                if handle_command(
                    command,
                    factory,
                    &mut model,
                    &mut core,
                    &mut observer,
                    &mut delivery,
                    &mut audio,
                ) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    audio.shutdown_all();
    if let Some(active) = core.as_deref_mut() {
        active.clear_input_source(KEYBOARD_INPUT_SOURCE);
    }
}

fn pump_audio_runtime(
    model: &mut RuntimeModel,
    mut core: Option<&mut (dyn RuntimeCore + '_)>,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
    delivery: &mut FrameQueue,
    audio: &mut RuntimeAudio,
) -> Duration {
    let Some(output) = audio.output.as_deref() else {
        return Duration::ZERO;
    };
    let health = output.health();
    if health.flush_pending {
        return AUDIO_POLL_INTERVAL;
    }
    let marks = output.watermarks();
    match pacing_decision(health, marks) {
        PacingDecision::Prime => {
            let queued_before = health.queued_stereo_frames;
            for _ in 0..MAX_PRIME_BATCHES {
                run_tick(model, core.as_deref_mut(), observer, delivery, audio);
                if model.snapshot.phase != RuntimePhase::Running || audio.output.is_none() {
                    break;
                }
                let Some(active_output) = audio.output.as_deref() else {
                    break;
                };
                let health = active_output.health();
                if health.flush_pending
                    || health.queued_stereo_frames >= active_output.watermarks().target
                {
                    break;
                }
            }
            audio.start_playback_if_primed();
            let queued_after = audio
                .output
                .as_deref()
                .map_or(queued_before, |output| output.health().queued_stereo_frames);
            if queued_after == queued_before && audio.output.is_some() && !audio.playing {
                audio.disable(AudioBackendError::new(
                    crate::audio::AudioBackendErrorKind::Backend,
                    "core produced no audio while priming",
                ));
            }
            Duration::ZERO
        }
        PacingDecision::RunOneBatch => {
            run_tick(model, core, observer, delivery, audio);
            audio.start_playback_if_primed();
            Duration::ZERO
        }
        PacingDecision::Wait(wait) | PacingDecision::Backpressured(wait) => {
            audio.start_playback_if_primed();
            wait
        }
        PacingDecision::FallbackFrameDeadline => Duration::ZERO,
    }
}

fn advance_frame_deadline(deadline: Instant, now: Instant) -> Instant {
    let next = deadline + FRAME_INTERVAL;
    if next > now {
        next
    } else {
        now + FRAME_INTERVAL
    }
}

#[allow(clippy::too_many_lines)]
fn handle_command(
    command: RuntimeCommand,
    factory: &Arc<dyn CoreFactory>,
    model: &mut RuntimeModel,
    core: &mut Option<Box<dyn RuntimeCore>>,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
    delivery: &mut FrameQueue,
    audio: &mut RuntimeAudio,
) -> bool {
    match command {
        RuntimeCommand::Subscribe {
            observer: replacement,
            reply,
        } => {
            delivery.clear();
            *observer = Some(replacement);
            let result = publish_control(model, observer).map(|()| model.snapshot());
            let _ = reply.send(result);
        }
        RuntimeCommand::AcknowledgeFrame { sequence, reply } => {
            let result = acknowledge_frame(sequence, observer, delivery);
            let _ = reply.send(result);
        }
        RuntimeCommand::OpenRom { path, reply } => {
            audio.shutdown_output();
            if let Some(active) = core.as_deref_mut() {
                active.clear_input_source(KEYBOARD_INPUT_SOURCE);
            }
            *core = None;
            delivery.clear();
            model.begin_load();
            let _ = publish_control(model, observer);
            let result = load_rom(&path, factory, model, core, audio);
            let _ = publish_control(model, observer);
            let _ = reply.send(result);
        }
        RuntimeCommand::Start { reply } => {
            let result = model.start().map(|()| model.snapshot());
            if result.is_ok() {
                let _ = publish_control(model, observer);
            }
            let _ = reply.send(result);
        }
        RuntimeCommand::Pause { reply } => {
            audio.pause();
            delivery.clear();
            let result = model.pause().map(|()| model.snapshot());
            if result.is_ok() {
                let _ = publish_control(model, observer);
            }
            let _ = reply.send(result);
        }
        RuntimeCommand::Restart { reply } => {
            delivery.clear();
            audio.pause();
            let result = restart_core(model, core);
            if result.is_ok() {
                let _ = publish_control(model, observer);
            }
            let _ = reply.send(result);
        }
        RuntimeCommand::Close { reply } => {
            audio.shutdown_output();
            if let Some(active) = core.as_deref_mut() {
                active.clear_input_source(KEYBOARD_INPUT_SOURCE);
            }
            *core = None;
            delivery.clear();
            model.close();
            let _ = publish_control(model, observer);
            let _ = reply.send(Ok(model.snapshot()));
        }
        RuntimeCommand::SetKeyboardInput { buttons, reply } => {
            let result = set_keyboard_input(core.as_deref_mut(), buttons);
            let _ = reply.send(result);
        }
        RuntimeCommand::Snapshot { reply } => {
            let _ = reply.send(Ok(model.snapshot()));
        }
        #[cfg(test)]
        RuntimeCommand::RunTicks { count, reply } => {
            let result = if model.snapshot.phase == RuntimePhase::Running {
                for _ in 0..count {
                    run_tick(model, core.as_deref_mut(), observer, delivery, audio);
                }
                Ok(())
            } else {
                Err(RuntimeError::new(
                    RuntimeErrorCode::InvalidLifecycle,
                    "The runtime must be running to execute ticks.",
                ))
            };
            let _ = reply.send(result);
        }
        #[cfg(test)]
        RuntimeCommand::BufferedFrameCount { reply } => {
            let _ = reply.send(Ok(delivery.buffered_frame_count()));
        }
        RuntimeCommand::Shutdown { reply } => {
            audio.shutdown_all();
            if let Some(active) = core.as_deref_mut() {
                active.clear_input_source(KEYBOARD_INPUT_SOURCE);
            }
            *core = None;
            delivery.clear();
            model.close();
            let _ = reply.send(Ok(()));
            return true;
        }
    }
    false
}

fn load_rom(
    path: &PathBuf,
    factory: &Arc<dyn CoreFactory>,
    model: &mut RuntimeModel,
    core: &mut Option<Box<dyn RuntimeCore>>,
    audio: &mut RuntimeAudio,
) -> RuntimeResult<RuntimeSnapshot> {
    let bytes = fs::read(path).map_err(|_| {
        RuntimeError::new(
            RuntimeErrorCode::FileInaccessible,
            "The selected ROM file could not be read.",
        )
    });
    let bytes = match bytes {
        Ok(bytes) => bytes,
        Err(error) => {
            model.fail_load(error.clone());
            return Err(error);
        }
    };
    let file_name = path.file_name().map_or_else(
        || "Selected ROM".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    let candidate_output = audio.prepare_candidate();
    let mut candidate = factory.create();
    // PED-40 owns persisted battery loading and every save/flush path.
    match candidate.load_rom(&bytes, None) {
        Ok(metadata) => {
            model.finish_load(metadata, file_name);
            *core = Some(candidate);
            audio.activate(candidate_output);
            Ok(model.snapshot())
        }
        Err(error) => {
            if let Some(mut output) = candidate_output {
                output.shutdown();
            }
            let error = RuntimeError::from(error);
            model.fail_load(error.clone());
            Err(error)
        }
    }
}

fn restart_core(
    model: &mut RuntimeModel,
    core: &mut Option<Box<dyn RuntimeCore>>,
) -> RuntimeResult<RuntimeSnapshot> {
    model.require_loaded()?;
    let active = core.as_deref_mut().ok_or_else(runtime_unavailable)?;
    active.clear_input_source(KEYBOARD_INPUT_SOURCE);
    active.reset().map_err(RuntimeError::from)?;
    model.restart()?;
    Ok(model.snapshot())
}

fn set_keyboard_input(
    core: Option<&mut (dyn RuntimeCore + '_)>,
    buttons: Vec<RuntimeButton>,
) -> RuntimeResult<()> {
    let active = core.ok_or_else(|| {
        RuntimeError::new(RuntimeErrorCode::InvalidLifecycle, "No ROM is loaded.")
    })?;
    let mut state = JoypadState::default();
    for button in buttons {
        state.press(match button {
            RuntimeButton::Up => Button::Up,
            RuntimeButton::Down => Button::Down,
            RuntimeButton::Left => Button::Left,
            RuntimeButton::Right => Button::Right,
            RuntimeButton::A => Button::A,
            RuntimeButton::B => Button::B,
            RuntimeButton::Start => Button::Start,
            RuntimeButton::Select => Button::Select,
        });
    }
    active.set_input(KEYBOARD_INPUT_SOURCE, state);
    Ok(())
}

fn run_tick(
    model: &mut RuntimeModel,
    core: Option<&mut (dyn RuntimeCore + '_)>,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
    delivery: &mut FrameQueue,
    audio: &mut RuntimeAudio,
) {
    let Some(active) = core else {
        model.fail_load(runtime_unavailable());
        let _ = publish_control(model, observer);
        return;
    };
    let outcome = active.run_cycles(FRAME_CYCLE_BUDGET);
    let batch = active.drain_audio();
    audio.enqueue(&batch);
    match outcome {
        Ok(outcome) if outcome.frame_ready() => {
            if let Some(frame) = active.take_frame() {
                offer_frame(frame, observer, delivery);
            }
        }
        Ok(_) => {}
        Err(error) => {
            model.fail_load(RuntimeError::from(error));
            delivery.clear();
            audio.shutdown_output();
            let _ = publish_control(model, observer);
        }
    }
}

fn publish_control(
    model: &RuntimeModel,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
) -> RuntimeResult<()> {
    let Some(active) = observer.as_ref() else {
        return Ok(());
    };
    let result = active.publish_control(RuntimeEvent::Snapshot {
        snapshot: model.snapshot(),
    });
    if result.is_err() {
        *observer = None;
    }
    result
}

fn offer_frame(
    frame: Frame,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
    delivery: &mut FrameQueue,
) {
    if observer.is_none() {
        delivery.clear();
        return;
    }
    if let Some(frame) = delivery.offer(frame) {
        publish_frame(&frame, observer, delivery);
    }
}

fn publish_frame(
    frame: &Frame,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
    delivery: &mut FrameQueue,
) {
    let Some(active) = observer.as_ref() else {
        delivery.clear();
        return;
    };
    if active.publish_frame(encode_frame_packet(frame)).is_err() {
        *observer = None;
        delivery.clear();
    }
}

fn acknowledge_frame(
    sequence: u64,
    observer: &mut Option<Arc<dyn RuntimeObserver>>,
    delivery: &mut FrameQueue,
) -> RuntimeResult<()> {
    let next = delivery
        .acknowledge(sequence)
        .map_err(|error| match error {
            AcknowledgeError::NotInFlight => RuntimeError::new(
                RuntimeErrorCode::InvalidLifecycle,
                "The acknowledged frame is not awaiting presentation.",
            ),
        })?;
    if let Some(frame) = next {
        publish_frame(&frame, observer, delivery);
    }
    Ok(())
}

fn runtime_unavailable() -> RuntimeError {
    RuntimeError::new(
        RuntimeErrorCode::RuntimeUnavailable,
        "The desktop runtime is unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::num::NonZeroU32;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Condvar, Mutex, mpsc};
    use std::thread;
    use std::time::{Duration, Instant};

    use gb_core::{
        AudioBatch, BatteryState, Button, CartridgeMetadata, CompatibilityMode, CoreError,
        EmulatorCore, Frame, InputSourceId, JoypadState, MapperKind, RunOutcome,
    };

    use super::{
        CoreFactory, DesktopRuntime, RuntimeAudio, RuntimeCore, RuntimeModel, RuntimeObserver,
    };
    use crate::audio::{
        AudioBackendError, AudioBackendErrorKind, AudioHealth, AudioOutput, AudioOutputFactory,
        AudioWatermarks,
    };
    use crate::emulator::contracts::{
        RuntimeButton, RuntimeError, RuntimeErrorCode, RuntimeEvent, RuntimePhase, RuntimeSnapshot,
    };
    use crate::emulator::mock_core::ContractMockCoreFactory;

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct NoAudioFactory;

    impl AudioOutputFactory for NoAudioFactory {
        fn open_default(&self) -> Result<Box<dyn AudioOutput>, AudioBackendError> {
            Err(AudioBackendError::new(
                AudioBackendErrorKind::NoOutputDevice,
                "no output in runtime tests",
            ))
        }
    }

    fn test_rate() -> NonZeroU32 {
        NonZeroU32::new(48_000).expect("non-zero test rate")
    }

    fn spawn_runtime(factory: Arc<dyn CoreFactory>) -> DesktopRuntime {
        DesktopRuntime::spawn_with_audio(factory, Arc::new(NoAudioFactory), test_rate(), None)
    }

    #[test]
    fn desktop_runtime_accepts_negotiated_audio_dependencies() {
        let rate = NonZeroU32::new(48_000).expect("non-zero rate");
        let runtime = DesktopRuntime::spawn_with_audio(
            Arc::new(RecordingCoreFactory::default()),
            Arc::new(NoAudioFactory),
            rate,
            None,
        );
        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn disabled_audio_still_validates_the_core_batch_rate() {
        let runtime_rate = test_rate();
        let mut audio = RuntimeAudio::new(
            Arc::new(NoAudioFactory),
            runtime_rate,
            None,
            Some(AudioBackendError::new(
                AudioBackendErrorKind::PermissionDenied,
                "preflight permission denial",
            )),
        );
        assert_eq!(
            audio.disabled_reason.as_ref().map(|reason| reason.kind),
            Some(AudioBackendErrorKind::PermissionDenied)
        );

        audio.enqueue(&AudioBatch::empty(
            NonZeroU32::new(44_100).expect("non-zero mismatched rate"),
        ));

        let reason = audio.disabled_reason.expect("typed fallback reason");
        assert_eq!(reason.kind, AudioBackendErrorKind::UnsupportedConfiguration);
        assert!(reason.message.contains("44100"));
        assert!(reason.message.contains("48000"));
    }

    #[test]
    fn shutdown_releases_prepared_audio_before_any_rom_is_loaded() {
        let rate = test_rate();
        let audio_factory = Arc::new(RecordingAudioFactory::new(rate));
        let (prepared, audio_state) = audio_factory.output();
        let runtime = DesktopRuntime::spawn_with_audio(
            Arc::new(RecordingCoreFactory::default()),
            audio_factory,
            rate,
            Some(prepared),
        );

        runtime.shutdown().expect("shutdown");

        let state = audio_state.lock().expect("prepared audio state");
        assert!(state.shutdown);
        assert_eq!(state.calls, ["audio.pause_flush", "audio.shutdown"]);
    }

    #[test]
    fn prepared_output_rate_mismatch_is_rejected_before_activation() {
        let path = synthetic_rom_path();
        let runtime_rate = test_rate();
        let audio_factory = Arc::new(RecordingAudioFactory::new(
            NonZeroU32::new(44_100).expect("non-zero mismatched rate"),
        ));
        let (prepared, audio_state) = audio_factory.output();
        let runtime = DesktopRuntime::spawn_with_audio(
            Arc::new(RecordingCoreFactory::default()),
            audio_factory,
            runtime_rate,
            Some(prepared),
        );

        runtime.open_rom(path.clone()).expect("ROM still loads");

        let state = audio_state.lock().expect("mismatched output state");
        assert!(state.shutdown);
        assert!(!state.playing);
        drop(state);
        runtime.start().expect("video-only fallback starts");
        assert_eq!(
            runtime.snapshot().expect("snapshot").phase,
            RuntimePhase::Running
        );
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[test]
    fn shutdown_with_active_rom_releases_audio_and_core_before_join() {
        let path = synthetic_rom_path();
        let rate = test_rate();
        let lifecycle = Arc::new(Mutex::new(Vec::new()));
        let core_factory = Arc::new(
            SyntheticAudioCoreFactory::new(rate, 1_000).with_lifecycle_log(Arc::clone(&lifecycle)),
        );
        let audio_factory =
            Arc::new(RecordingAudioFactory::new(rate).with_lifecycle_log(Arc::clone(&lifecycle)));
        let (prepared, audio_state) = audio_factory.output();
        let runtime =
            DesktopRuntime::spawn_with_audio(core_factory, audio_factory, rate, Some(prepared));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.start().expect("starts");
        wait_until(|| audio_state.lock().expect("audio state").playing);
        lifecycle.lock().expect("lifecycle").clear();

        runtime.shutdown().expect("shutdown joins worker");

        let lifecycle = lifecycle.lock().expect("lifecycle").clone();
        assert_eq!(
            lifecycle,
            [
                "audio.pause_flush",
                "audio.shutdown",
                "core.clear_input",
                "core.drop",
            ]
        );
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[test]
    fn negotiated_rate_reaches_every_drained_audio_batch() {
        let path = synthetic_rom_path();
        let rate = NonZeroU32::new(44_100).expect("non-zero rate");
        let core_factory = Arc::new(SyntheticAudioCoreFactory::new(rate, 900));
        let audio_factory = Arc::new(RecordingAudioFactory::new(rate));
        let (prepared, audio_state) = audio_factory.output();
        let runtime =
            DesktopRuntime::spawn_with_audio(core_factory, audio_factory, rate, Some(prepared));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.start().expect("starts");
        wait_until(|| audio_state.lock().expect("audio state").playing);

        let state = audio_state.lock().expect("audio state");
        assert!(!state.enqueue_rates.is_empty());
        assert!(state.enqueue_rates.iter().all(|actual| *actual == rate));
        assert!(state.max_queued <= AudioWatermarks::for_rate(rate).capacity);
        drop(state);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[test]
    fn missing_audio_uses_frame_deadlines_and_still_drains_core_audio() {
        let path = synthetic_rom_path();
        let rate = test_rate();
        let core_factory = Arc::new(SyntheticAudioCoreFactory::new(rate, 800));
        let core_state = Arc::clone(&core_factory.state);
        let runtime =
            DesktopRuntime::spawn_with_audio(core_factory, Arc::new(NoAudioFactory), rate, None);
        runtime.open_rom(path.clone()).expect("loads");
        runtime.start().expect("starts");
        thread::sleep(Duration::from_millis(55));
        runtime
            .set_keyboard_input(vec![RuntimeButton::A])
            .expect("input remains usable");
        assert_eq!(
            runtime.snapshot().expect("snapshot").phase,
            RuntimePhase::Running
        );

        let state = core_state.lock().expect("core state");
        assert!(
            (2..=5).contains(&state.run_calls),
            "fallback cadence: {}",
            state.run_calls
        );
        assert_eq!(state.drain_calls, state.run_calls);
        assert_eq!(state.inputs.len(), 1);
        drop(state);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[test]
    fn missing_audio_keeps_video_delivery_usable() {
        let path = synthetic_rom_path();
        let rate = test_rate();
        let runtime = DesktopRuntime::spawn_with_audio(
            Arc::new(ContractMockCoreFactory::with_sample_rate(rate)),
            Arc::new(NoAudioFactory),
            rate,
            None,
        );
        let observer = RecordingObserver::default();
        runtime
            .subscribe(Arc::new(observer.clone()))
            .expect("subscribe");
        runtime.open_rom(path.clone()).expect("loads");
        runtime.start().expect("starts");

        wait_until(|| !observer.frames.lock().expect("frames").is_empty());

        assert_eq!(
            runtime.snapshot().expect("snapshot").phase,
            RuntimePhase::Running
        );
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[test]
    fn start_primes_to_target_in_four_batches_and_high_watermark_backpressures() {
        let path = synthetic_rom_path();
        let rate = test_rate();
        let core_factory = Arc::new(SyntheticAudioCoreFactory::new(rate, 1_000));
        let core_state = Arc::clone(&core_factory.state);
        let audio_factory = Arc::new(RecordingAudioFactory::new(rate));
        let (prepared, audio_state) = audio_factory.output();
        let runtime =
            DesktopRuntime::spawn_with_audio(core_factory, audio_factory, rate, Some(prepared));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.start().expect("starts");
        wait_until(|| audio_state.lock().expect("audio state").playing);
        assert_eq!(core_state.lock().expect("core state").run_calls, 4);
        assert_eq!(audio_state.lock().expect("audio state").queued, 4_000);
        runtime.shutdown().expect("shutdown");

        let core_factory = Arc::new(SyntheticAudioCoreFactory::new(rate, 1_000));
        let core_state = Arc::clone(&core_factory.state);
        let audio_factory = Arc::new(RecordingAudioFactory::new(rate));
        let (prepared, audio_state) = audio_factory.output();
        audio_state.lock().expect("audio state").queued = AudioWatermarks::for_rate(rate).high + 1;
        let runtime =
            DesktopRuntime::spawn_with_audio(core_factory, audio_factory, rate, Some(prepared));
        runtime.open_rom(path.clone()).expect("loads again");
        runtime.start().expect("starts again");
        wait_until(|| audio_state.lock().expect("audio state").playing);
        thread::sleep(Duration::from_millis(15));
        assert_eq!(core_state.lock().expect("core state").run_calls, 0);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[test]
    fn pause_restart_close_flush_and_release_audio_in_order() {
        let path = synthetic_rom_path();
        let rate = test_rate();
        let lifecycle = Arc::new(Mutex::new(Vec::new()));
        let core_factory = Arc::new(
            SyntheticAudioCoreFactory::new(rate, 1_000).with_lifecycle_log(Arc::clone(&lifecycle)),
        );
        let core_state = Arc::clone(&core_factory.state);
        let audio_factory =
            Arc::new(RecordingAudioFactory::new(rate).with_lifecycle_log(Arc::clone(&lifecycle)));
        let (prepared, audio_state) = audio_factory.output();
        let runtime =
            DesktopRuntime::spawn_with_audio(core_factory, audio_factory, rate, Some(prepared));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.start().expect("starts");
        wait_until(|| audio_state.lock().expect("audio state").playing);
        lifecycle.lock().expect("lifecycle").clear();

        runtime.pause().expect("pauses");
        let runs_after_pause = core_state.lock().expect("core state").run_calls;
        thread::sleep(Duration::from_millis(20));
        assert_eq!(
            core_state.lock().expect("core state").run_calls,
            runs_after_pause
        );
        wait_until(|| !audio_state.lock().expect("audio state").flush_pending);
        lifecycle.lock().expect("lifecycle").clear();

        runtime.restart().expect("restarts");
        wait_until(|| core_state.lock().expect("core state").reset_calls == 1);
        wait_until(|| {
            audio_state
                .lock()
                .expect("audio state")
                .calls
                .iter()
                .filter(|call| **call == "audio.play")
                .count()
                == 2
        });
        let calls = audio_state.lock().expect("audio state").calls.clone();
        let pause = calls
            .iter()
            .rposition(|call| *call == "audio.pause_flush")
            .expect("pause recorded");
        let ack = calls
            .iter()
            .enumerate()
            .skip(pause + 1)
            .find_map(|(index, call)| (*call == "audio.flush_ack").then_some(index))
            .expect("restart flush acknowledged");
        let play = calls
            .iter()
            .rposition(|call| *call == "audio.play")
            .expect("resume play recorded");
        assert!(pause < ack && ack < play, "calls: {calls:?}");
        let restart_lifecycle = lifecycle.lock().expect("lifecycle").clone();
        assert_eq!(
            restart_lifecycle,
            [
                "audio.pause_flush",
                "core.clear_input",
                "core.reset",
                "audio.flush_ack",
                "audio.play",
            ]
        );

        lifecycle.lock().expect("lifecycle").clear();
        runtime.close().expect("closes");
        let state = audio_state.lock().expect("audio state");
        assert!(state.shutdown);
        assert_eq!(state.queued, 0);
        assert!(
            state
                .calls
                .ends_with(&["audio.pause_flush", "audio.shutdown"])
        );
        drop(state);
        let close_lifecycle = lifecycle.lock().expect("lifecycle").clone();
        assert_eq!(
            close_lifecycle,
            [
                "audio.pause_flush",
                "audio.shutdown",
                "core.clear_input",
                "core.drop",
            ]
        );
        runtime.shutdown().expect("idempotent shutdown");
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[test]
    fn terminal_audio_failure_drops_output_but_keeps_runtime_running() {
        let path = synthetic_rom_path();
        let rate = test_rate();
        let core_factory = Arc::new(SyntheticAudioCoreFactory::new(rate, 1_000));
        let core_state = Arc::clone(&core_factory.state);
        let audio_factory = Arc::new(RecordingAudioFactory::new(rate));
        let (prepared, audio_state) = audio_factory.output();
        let runtime =
            DesktopRuntime::spawn_with_audio(core_factory, audio_factory, rate, Some(prepared));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.start().expect("starts");
        wait_until(|| audio_state.lock().expect("audio state").playing);
        audio_state.lock().expect("audio state").usable = false;
        wait_until(|| audio_state.lock().expect("audio state").shutdown);
        let runs_at_failure = core_state.lock().expect("core state").run_calls;
        thread::sleep(Duration::from_millis(40));
        assert!(core_state.lock().expect("core state").run_calls > runs_at_failure);
        assert_eq!(
            runtime.snapshot().expect("snapshot").phase,
            RuntimePhase::Running
        );
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[test]
    fn fatal_core_failure_stops_and_releases_audio_output() {
        let path = synthetic_rom_path();
        let rate = test_rate();
        let core_factory = Arc::new(SyntheticAudioCoreFactory::new(rate, 1_000));
        let core_state = Arc::clone(&core_factory.state);
        let audio_factory = Arc::new(RecordingAudioFactory::new(rate).with_consumption(1_000));
        let (prepared, audio_state) = audio_factory.output();
        let runtime =
            DesktopRuntime::spawn_with_audio(core_factory, audio_factory, rate, Some(prepared));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.start().expect("starts");
        wait_until(|| audio_state.lock().expect("audio state").playing);

        core_state.lock().expect("core state").fail_run = true;
        wait_until(|| audio_state.lock().expect("audio state").shutdown);

        assert_eq!(
            runtime.snapshot().expect("snapshot").phase,
            RuntimePhase::Error
        );
        assert!(
            audio_state
                .lock()
                .expect("audio state")
                .calls
                .ends_with(&["audio.pause_flush", "audio.shutdown"])
        );
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[test]
    fn mismatched_core_audio_rate_is_rejected_without_resampling() {
        let path = synthetic_rom_path();
        let runtime_rate = test_rate();
        let core_rate = NonZeroU32::new(44_100).expect("non-zero core rate");
        let core_factory = Arc::new(SyntheticAudioCoreFactory::new(core_rate, 900));
        let audio_factory = Arc::new(RecordingAudioFactory::new(runtime_rate));
        let (prepared, audio_state) = audio_factory.output();
        let runtime = DesktopRuntime::spawn_with_audio(
            core_factory,
            audio_factory,
            runtime_rate,
            Some(prepared),
        );
        runtime.open_rom(path.clone()).expect("loads");
        runtime.start().expect("starts");
        wait_until(|| audio_state.lock().expect("audio state").shutdown);
        let state = audio_state.lock().expect("audio state");
        assert!(state.enqueue_rates.is_empty());
        drop(state);
        assert_eq!(
            runtime.snapshot().expect("snapshot").phase,
            RuntimePhase::Running
        );
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[test]
    fn rom_replacement_releases_old_audio_and_core_before_activating_new_ones() {
        let first = synthetic_rom_path();
        let second = synthetic_rom_path();
        let rate = test_rate();
        let lifecycle = Arc::new(Mutex::new(Vec::new()));
        let core_factory = Arc::new(
            SyntheticAudioCoreFactory::new(rate, 1_000).with_lifecycle_log(Arc::clone(&lifecycle)),
        );
        let core_state = Arc::clone(&core_factory.state);
        let audio_factory =
            Arc::new(RecordingAudioFactory::new(rate).with_lifecycle_log(Arc::clone(&lifecycle)));
        let opened = Arc::clone(&audio_factory.opened);
        let (prepared, first_audio_state) = audio_factory.output();
        let runtime =
            DesktopRuntime::spawn_with_audio(core_factory, audio_factory, rate, Some(prepared));
        runtime.open_rom(first.clone()).expect("first loads");
        runtime.start().expect("first starts");
        wait_until(|| first_audio_state.lock().expect("first audio").playing);
        lifecycle.lock().expect("lifecycle").clear();

        runtime.open_rom(second.clone()).expect("replacement loads");
        assert!(first_audio_state.lock().expect("first audio").shutdown);
        assert_eq!(core_state.lock().expect("core state").dropped_cores, 1);
        assert_eq!(opened.lock().expect("opened outputs").len(), 1);
        let replacement_lifecycle = lifecycle.lock().expect("lifecycle").clone();
        assert_eq!(
            replacement_lifecycle,
            [
                "audio.pause_flush",
                "audio.shutdown",
                "core.clear_input",
                "core.drop",
                "audio.open",
                "core.create",
            ]
        );
        runtime.close().expect("close replacement");
        assert_eq!(core_state.lock().expect("core state").dropped_cores, 2);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(first).expect("remove first ROM");
        fs::remove_file(second).expect("remove second ROM");
    }

    #[test]
    #[ignore = "deterministic 30-minute synthetic runtime soak"]
    fn synthetic_audio_soak_30_minutes() {
        let path = synthetic_rom_path();
        let rate = test_rate();
        let core_factory = Arc::new(SyntheticAudioCoreFactory::new(rate, 1_000));
        let audio_factory = Arc::new(RecordingAudioFactory::new(rate).with_consumption(512));
        let opened = Arc::clone(&audio_factory.opened);
        let (prepared, prepared_state) = audio_factory.output();
        let core_state = Arc::clone(&core_factory.state);
        let runtime =
            DesktopRuntime::spawn_with_audio(core_factory, audio_factory, rate, Some(prepared));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.start().expect("starts");

        let started = Instant::now();
        let deadline = started + Duration::from_mins(30) + Duration::from_secs(5);
        let mut next_sample = started + Duration::from_secs(60);
        let mut samples = 0;
        let mut pauses = 0;
        let mut restarts = 0;
        let mut replacements = 0;
        let mut last_run_calls = 0;
        while samples < 30 {
            assert!(
                Instant::now() < deadline,
                "soak exceeded its bounded deadline"
            );
            if Instant::now() >= next_sample {
                samples += 1;
                let capacity = AudioWatermarks::for_rate(rate).capacity;
                assert!(prepared_state.lock().expect("prepared audio").max_queued <= capacity);
                for state in opened.lock().expect("opened outputs").iter() {
                    assert!(state.lock().expect("opened audio").max_queued <= capacity);
                }
                let run_calls = core_state.lock().expect("core state").run_calls;
                assert!(
                    run_calls > last_run_calls,
                    "emulation stopped making progress"
                );
                last_run_calls = run_calls;
                if samples <= 10 {
                    runtime.pause().expect("soak pause");
                    runtime.start().expect("soak resume");
                    pauses += 1;
                }
                if samples <= 5 {
                    runtime.restart().expect("soak restart");
                    restarts += 1;
                }
                if matches!(samples, 10 | 20 | 30) {
                    runtime.open_rom(path.clone()).expect("soak replacement");
                    runtime.start().expect("replacement starts");
                    replacements += 1;
                }
                next_sample += Duration::from_secs(60);
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!((samples, pauses, restarts, replacements), (30, 10, 5, 3));
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[derive(Default)]
    struct RecordingState {
        created: usize,
        dropped: usize,
        fail_load: Option<CoreError>,
        inputs: Vec<(InputSourceId, JoypadState)>,
        clears: Vec<InputSourceId>,
    }

    struct RecordingCore {
        state: Arc<Mutex<RecordingState>>,
        loaded: bool,
    }

    impl Drop for RecordingCore {
        fn drop(&mut self) {
            self.state.lock().expect("recording state").dropped += 1;
        }
    }

    impl EmulatorCore for RecordingCore {
        fn load_rom(
            &mut self,
            _rom: &[u8],
            _persisted: Option<&BatteryState>,
        ) -> Result<CartridgeMetadata, CoreError> {
            if let Some(error) = self
                .state
                .lock()
                .expect("recording state")
                .fail_load
                .clone()
            {
                return Err(error);
            }
            self.loaded = true;
            Ok(test_metadata())
        }

        fn reset(&mut self) -> Result<(), CoreError> {
            self.loaded.then_some(()).ok_or(CoreError::NotLoaded)
        }

        fn run_cycles(&mut self, cycle_budget: u32) -> Result<RunOutcome, CoreError> {
            self.loaded
                .then(|| RunOutcome::idle(cycle_budget))
                .ok_or(CoreError::NotLoaded)
        }

        fn set_input(&mut self, source: InputSourceId, state: JoypadState) {
            self.state
                .lock()
                .expect("recording state")
                .inputs
                .push((source, state));
        }

        fn clear_input_source(&mut self, source: InputSourceId) {
            self.state
                .lock()
                .expect("recording state")
                .clears
                .push(source);
        }

        fn take_frame(&mut self) -> Option<Frame> {
            None
        }

        fn drain_audio(&mut self) -> AudioBatch {
            AudioBatch::empty(NonZeroU32::new(48_000).expect("non-zero rate"))
        }

        fn battery_state(&self) -> Option<BatteryState> {
            None
        }
    }

    #[derive(Default)]
    struct RecordingCoreFactory {
        state: Arc<Mutex<RecordingState>>,
    }

    impl CoreFactory for RecordingCoreFactory {
        fn create(&self) -> Box<dyn RuntimeCore> {
            self.state.lock().expect("recording state").created += 1;
            Box::new(RecordingCore {
                state: Arc::clone(&self.state),
                loaded: false,
            })
        }
    }

    #[derive(Default)]
    struct SyntheticAudioState {
        run_calls: usize,
        drain_calls: usize,
        reset_calls: usize,
        dropped_cores: usize,
        fail_run: bool,
        inputs: Vec<(InputSourceId, JoypadState)>,
    }

    struct SyntheticAudioCoreFactory {
        sample_rate: NonZeroU32,
        frames_per_tick: usize,
        state: Arc<Mutex<SyntheticAudioState>>,
        lifecycle_log: Option<Arc<Mutex<Vec<&'static str>>>>,
    }

    impl SyntheticAudioCoreFactory {
        fn new(sample_rate: NonZeroU32, frames_per_tick: usize) -> Self {
            Self {
                sample_rate,
                frames_per_tick,
                state: Arc::default(),
                lifecycle_log: None,
            }
        }

        fn with_lifecycle_log(mut self, log: Arc<Mutex<Vec<&'static str>>>) -> Self {
            self.lifecycle_log = Some(log);
            self
        }
    }

    impl CoreFactory for SyntheticAudioCoreFactory {
        fn create(&self) -> Box<dyn RuntimeCore> {
            record_lifecycle(self.lifecycle_log.as_ref(), "core.create");
            Box::new(SyntheticAudioCore {
                sample_rate: self.sample_rate,
                frames_per_tick: self.frames_per_tick,
                state: Arc::clone(&self.state),
                loaded: false,
                pending: None,
                lifecycle_log: self.lifecycle_log.clone(),
            })
        }
    }

    struct SyntheticAudioCore {
        sample_rate: NonZeroU32,
        frames_per_tick: usize,
        state: Arc<Mutex<SyntheticAudioState>>,
        loaded: bool,
        pending: Option<AudioBatch>,
        lifecycle_log: Option<Arc<Mutex<Vec<&'static str>>>>,
    }

    impl Drop for SyntheticAudioCore {
        fn drop(&mut self) {
            record_lifecycle(self.lifecycle_log.as_ref(), "core.drop");
            self.state.lock().expect("synthetic state").dropped_cores += 1;
        }
    }

    impl EmulatorCore for SyntheticAudioCore {
        fn load_rom(
            &mut self,
            rom: &[u8],
            _persisted: Option<&BatteryState>,
        ) -> Result<CartridgeMetadata, CoreError> {
            if rom.is_empty() {
                return Err(CoreError::InvalidRom("empty synthetic ROM".into()));
            }
            self.loaded = true;
            self.pending = None;
            Ok(test_metadata())
        }

        fn reset(&mut self) -> Result<(), CoreError> {
            if !self.loaded {
                return Err(CoreError::NotLoaded);
            }
            self.pending = None;
            self.state.lock().expect("synthetic state").reset_calls += 1;
            record_lifecycle(self.lifecycle_log.as_ref(), "core.reset");
            Ok(())
        }

        fn run_cycles(&mut self, cycle_budget: u32) -> Result<RunOutcome, CoreError> {
            if !self.loaded {
                return Err(CoreError::NotLoaded);
            }
            let mut state = self.state.lock().expect("synthetic state");
            state.run_calls += 1;
            if state.fail_run {
                return Err(CoreError::InternalInvariant("synthetic run failure".into()));
            }
            drop(state);
            let samples = (0..self.frames_per_tick)
                .flat_map(|_| [0.25, -0.25])
                .collect();
            self.pending = Some(
                AudioBatch::new(self.sample_rate, samples).expect("valid synthetic audio batch"),
            );
            Ok(RunOutcome::new(cycle_budget, false, self.frames_per_tick))
        }

        fn set_input(&mut self, source: InputSourceId, state: JoypadState) {
            self.state
                .lock()
                .expect("synthetic state")
                .inputs
                .push((source, state));
        }

        fn clear_input_source(&mut self, _source: InputSourceId) {
            record_lifecycle(self.lifecycle_log.as_ref(), "core.clear_input");
        }

        fn take_frame(&mut self) -> Option<Frame> {
            None
        }

        fn drain_audio(&mut self) -> AudioBatch {
            self.state.lock().expect("synthetic state").drain_calls += 1;
            self.pending
                .take()
                .unwrap_or_else(|| AudioBatch::empty(self.sample_rate))
        }

        fn battery_state(&self) -> Option<BatteryState> {
            None
        }
    }

    #[allow(clippy::struct_excessive_bools)]
    struct FakeAudioState {
        queued: usize,
        max_queued: usize,
        flush_pending: bool,
        auto_ack_flush: bool,
        usable: bool,
        playing: bool,
        consume_per_health: usize,
        enqueue_rates: Vec<NonZeroU32>,
        calls: Vec<&'static str>,
        shutdown: bool,
    }

    impl FakeAudioState {
        fn new() -> Self {
            Self {
                queued: 0,
                max_queued: 0,
                flush_pending: false,
                auto_ack_flush: true,
                usable: true,
                playing: false,
                consume_per_health: 0,
                enqueue_rates: Vec::new(),
                calls: Vec::new(),
                shutdown: false,
            }
        }
    }

    struct FakeAudioOutput {
        sample_rate: NonZeroU32,
        state: Arc<Mutex<FakeAudioState>>,
        lifecycle_log: Option<Arc<Mutex<Vec<&'static str>>>>,
    }

    impl AudioOutput for FakeAudioOutput {
        fn sample_rate(&self) -> NonZeroU32 {
            self.sample_rate
        }

        fn watermarks(&self) -> AudioWatermarks {
            AudioWatermarks::for_rate(self.sample_rate)
        }

        fn enqueue(&mut self, batch: &AudioBatch) -> Result<(), AudioBackendError> {
            let mut state = self.state.lock().expect("fake audio state");
            let next = state.queued + batch.stereo_frame_count();
            if next > self.watermarks().capacity {
                return Err(AudioBackendError::new(
                    AudioBackendErrorKind::DeviceBusy,
                    "fake queue capacity exceeded",
                ));
            }
            state.queued = next;
            state.max_queued = state.max_queued.max(next);
            state.enqueue_rates.push(batch.sample_rate());
            state.calls.push("audio.enqueue");
            Ok(())
        }

        fn health(&self) -> AudioHealth {
            let mut state = self.state.lock().expect("fake audio state");
            if state.flush_pending && state.auto_ack_flush {
                state.queued = 0;
                state.flush_pending = false;
                state.calls.push("audio.flush_ack");
                record_lifecycle(self.lifecycle_log.as_ref(), "audio.flush_ack");
            }
            if state.playing {
                state.queued = state.queued.saturating_sub(state.consume_per_health);
            }
            AudioHealth {
                queued_stereo_frames: state.queued,
                flush_pending: state.flush_pending,
                underruns: 0,
                dropped_stereo_frames: 0,
                stream_errors: usize::from(!state.usable) as u64,
                usable: state.usable,
            }
        }

        fn play(&mut self) -> Result<(), AudioBackendError> {
            let mut state = self.state.lock().expect("fake audio state");
            state.playing = true;
            state.calls.push("audio.play");
            record_lifecycle(self.lifecycle_log.as_ref(), "audio.play");
            Ok(())
        }

        fn pause_and_flush(&mut self) -> Result<(), AudioBackendError> {
            let mut state = self.state.lock().expect("fake audio state");
            state.playing = false;
            state.flush_pending = true;
            state.calls.push("audio.pause_flush");
            record_lifecycle(self.lifecycle_log.as_ref(), "audio.pause_flush");
            Ok(())
        }

        fn shutdown(&mut self) {
            let mut state = self.state.lock().expect("fake audio state");
            if !state.shutdown {
                state.shutdown = true;
                state.playing = false;
                state.queued = 0;
                state.calls.push("audio.shutdown");
                record_lifecycle(self.lifecycle_log.as_ref(), "audio.shutdown");
            }
        }
    }

    struct RecordingAudioFactory {
        sample_rate: NonZeroU32,
        consume_per_health: usize,
        opened: Arc<Mutex<Vec<Arc<Mutex<FakeAudioState>>>>>,
        lifecycle_log: Option<Arc<Mutex<Vec<&'static str>>>>,
    }

    impl RecordingAudioFactory {
        fn new(sample_rate: NonZeroU32) -> Self {
            Self {
                sample_rate,
                consume_per_health: 0,
                opened: Arc::default(),
                lifecycle_log: None,
            }
        }

        fn with_consumption(mut self, stereo_frames: usize) -> Self {
            self.consume_per_health = stereo_frames;
            self
        }

        fn with_lifecycle_log(mut self, log: Arc<Mutex<Vec<&'static str>>>) -> Self {
            self.lifecycle_log = Some(log);
            self
        }

        fn output(&self) -> (Box<dyn AudioOutput>, Arc<Mutex<FakeAudioState>>) {
            let mut state = FakeAudioState::new();
            state.consume_per_health = self.consume_per_health;
            let state = Arc::new(Mutex::new(state));
            (
                Box::new(FakeAudioOutput {
                    sample_rate: self.sample_rate,
                    state: Arc::clone(&state),
                    lifecycle_log: self.lifecycle_log.clone(),
                }),
                state,
            )
        }
    }

    impl AudioOutputFactory for RecordingAudioFactory {
        fn open_default(&self) -> Result<Box<dyn AudioOutput>, AudioBackendError> {
            record_lifecycle(self.lifecycle_log.as_ref(), "audio.open");
            let (output, state) = self.output();
            self.opened.lock().expect("opened outputs").push(state);
            Ok(output)
        }
    }

    fn record_lifecycle(log: Option<&Arc<Mutex<Vec<&'static str>>>>, event: &'static str) {
        if let Some(log) = log {
            log.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(event);
        }
    }

    fn wait_until(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while !condition() {
            assert!(Instant::now() < deadline, "condition did not become true");
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn synthetic_rom_path() -> PathBuf {
        let id = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ped-37-{}-{id}.gb", std::process::id()));
        fs::write(&path, b"PED-37 synthetic ROM").expect("write synthetic ROM");
        path
    }

    #[derive(Clone, Default)]
    struct RecordingObserver {
        control: Arc<Mutex<Vec<RuntimeEvent>>>,
        frames: Arc<Mutex<Vec<Vec<u8>>>>,
        fail_frames: Arc<Mutex<bool>>,
    }

    impl RecordingObserver {
        fn frame_sequences(&self) -> Vec<u64> {
            self.frames
                .lock()
                .expect("frames")
                .iter()
                .map(|packet| u64::from_le_bytes(packet[0..8].try_into().expect("sequence header")))
                .collect()
        }
    }

    impl RuntimeObserver for RecordingObserver {
        fn publish_control(&self, event: RuntimeEvent) -> Result<(), RuntimeError> {
            self.control.lock().expect("control").push(event);
            Ok(())
        }

        fn publish_frame(&self, packet: Vec<u8>) -> Result<(), RuntimeError> {
            if *self.fail_frames.lock().expect("failure flag") {
                return Err(RuntimeError::new(
                    RuntimeErrorCode::RuntimeUnavailable,
                    "observer unavailable",
                ));
            }
            self.frames.lock().expect("frames").push(packet);
            Ok(())
        }
    }

    struct FrameSignalObserver {
        frames: mpsc::SyncSender<u64>,
    }

    impl RuntimeObserver for FrameSignalObserver {
        fn publish_control(&self, _event: RuntimeEvent) -> Result<(), RuntimeError> {
            Ok(())
        }

        fn publish_frame(&self, packet: Vec<u8>) -> Result<(), RuntimeError> {
            let sequence = u64::from_le_bytes(packet[0..8].try_into().expect("sequence header"));
            self.frames.try_send(sequence).map_err(|_| {
                RuntimeError::new(
                    RuntimeErrorCode::RuntimeUnavailable,
                    "frame signal unavailable",
                )
            })
        }
    }

    struct BlockingControlObserver {
        entered: mpsc::SyncSender<()>,
        release: Arc<(Mutex<bool>, Condvar)>,
    }

    impl RuntimeObserver for BlockingControlObserver {
        fn publish_control(&self, _event: RuntimeEvent) -> Result<(), RuntimeError> {
            self.entered.send(()).expect("test receives worker entry");
            let (lock, condition) = &*self.release;
            let released = lock.lock().expect("release gate");
            let _released = condition
                .wait_while(released, |released| !*released)
                .expect("release gate remains available");
            Ok(())
        }

        fn publish_frame(&self, _packet: Vec<u8>) -> Result<(), RuntimeError> {
            Ok(())
        }
    }

    fn test_metadata() -> CartridgeMetadata {
        CartridgeMetadata {
            title: "Fixture".into(),
            rom_identity: "fixture".into(),
            mapper: MapperKind::RomOnly,
            compatibility: CompatibilityMode::Dmg,
            ram_size_bytes: 0,
            has_battery: false,
        }
    }

    #[test]
    fn lifecycle_transitions_are_explicit() {
        let mut model = RuntimeModel::default();
        assert_eq!(model.snapshot().phase, RuntimePhase::Empty);

        model.begin_load();
        assert_eq!(model.snapshot().phase, RuntimePhase::Loading);

        model.finish_load(test_metadata(), "fixture.gb".into());
        assert_eq!(model.snapshot().phase, RuntimePhase::Paused);

        model.start().expect("loaded ROM starts");
        assert_eq!(model.snapshot().phase, RuntimePhase::Running);

        model.pause().expect("running ROM pauses");
        assert_eq!(model.snapshot().phase, RuntimePhase::Paused);

        model.restart().expect("loaded ROM restarts");
        assert_eq!(model.snapshot().phase, RuntimePhase::Running);

        model.close();
        assert_eq!(model.snapshot(), RuntimeSnapshot::empty());
    }

    #[test]
    fn lifecycle_idempotence_and_errors_are_explicit() {
        let mut model = RuntimeModel::default();
        let error = model.start().expect_err("empty runtime cannot start");
        assert_eq!(error.code, RuntimeErrorCode::InvalidLifecycle);

        model.begin_load();
        model.finish_load(test_metadata(), "fixture.gb".into());
        model.pause().expect("already paused is idempotent");
        model.start().expect("loaded runtime starts");
        model.start().expect("already running is idempotent");
        assert_eq!(model.snapshot().phase, RuntimePhase::Running);
    }

    #[test]
    fn runtime_audio_lifecycle_operations_are_repeatable() {
        let path = synthetic_rom_path();
        let rate = test_rate();
        let core_factory = Arc::new(SyntheticAudioCoreFactory::new(rate, 1_000));
        let audio_factory = Arc::new(RecordingAudioFactory::new(rate));
        let (prepared, audio_state) = audio_factory.output();
        let runtime =
            DesktopRuntime::spawn_with_audio(core_factory, audio_factory, rate, Some(prepared));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.start().expect("starts");
        wait_until(|| audio_state.lock().expect("audio state").playing);

        runtime.pause().expect("first pause");
        runtime.pause().expect("second pause");
        runtime.restart().expect("first restart");
        runtime.restart().expect("second restart");
        runtime.close().expect("first close");
        runtime.close().expect("second close");

        assert!(audio_state.lock().expect("audio state").shutdown);
        assert_eq!(
            runtime.snapshot().expect("snapshot").phase,
            RuntimePhase::Empty
        );
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove synthetic ROM");
    }

    #[test]
    fn lifecycle_load_error_clears_rom_metadata() {
        let mut model = RuntimeModel::default();
        model.begin_load();
        model.fail_load(crate::emulator::contracts::RuntimeError::new(
            RuntimeErrorCode::InvalidRom,
            "bad fixture",
        ));

        assert_eq!(model.snapshot().phase, RuntimePhase::Error);
        assert!(model.snapshot().rom.is_none());
        assert_eq!(
            model.snapshot().error.expect("typed error").code,
            RuntimeErrorCode::InvalidRom
        );
    }

    #[test]
    fn desktop_runtime_serializes_rom_lifecycle() {
        let path = synthetic_rom_path();
        let factory = Arc::new(RecordingCoreFactory::default());
        let runtime = spawn_runtime(factory);

        let loaded = runtime.open_rom(path.clone()).expect("synthetic ROM loads");
        assert_eq!(loaded.phase, RuntimePhase::Paused);
        assert_eq!(
            loaded.rom.expect("summary").file_name,
            path.file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned()
        );
        assert_eq!(runtime.start().expect("start").phase, RuntimePhase::Running);
        assert_eq!(runtime.pause().expect("pause").phase, RuntimePhase::Paused);
        assert_eq!(
            runtime.restart().expect("restart").phase,
            RuntimePhase::Running
        );
        assert_eq!(runtime.close().expect("close").phase, RuntimePhase::Empty);
        runtime.shutdown().expect("shutdown joins worker");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn desktop_runtime_maps_file_and_core_failures() {
        let runtime = spawn_runtime(Arc::new(RecordingCoreFactory::default()));
        let missing = std::env::temp_dir().join("ped-37-definitely-missing.gb");
        let error = runtime.open_rom(missing).expect_err("missing file fails");
        assert_eq!(error.code, RuntimeErrorCode::FileInaccessible);
        runtime.shutdown().expect("shutdown");

        let path = synthetic_rom_path();
        let factory = Arc::new(RecordingCoreFactory::default());
        factory.state.lock().expect("state").fail_load = Some(CoreError::UnsupportedMapper(0x42));
        let runtime = spawn_runtime(factory);
        let error = runtime
            .open_rom(path.clone())
            .expect_err("core rejects ROM");
        assert_eq!(error.code, RuntimeErrorCode::UnsupportedMapper);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn desktop_runtime_replacement_drops_the_previous_core() {
        let first = synthetic_rom_path();
        let second = synthetic_rom_path();
        let factory = Arc::new(RecordingCoreFactory::default());
        let state = Arc::clone(&factory.state);
        let runtime = spawn_runtime(factory);
        runtime.open_rom(first.clone()).expect("first loads");
        runtime.open_rom(second.clone()).expect("second loads");

        let state = state.lock().expect("state");
        assert_eq!(state.created, 2);
        assert_eq!(state.dropped, 1);
        drop(state);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(first).expect("remove first");
        fs::remove_file(second).expect("remove second");
    }

    #[test]
    fn desktop_runtime_sends_complete_keyboard_snapshots_and_clears_on_close() {
        let path = synthetic_rom_path();
        let factory = Arc::new(RecordingCoreFactory::default());
        let state = Arc::clone(&factory.state);
        let runtime = spawn_runtime(factory);
        runtime.open_rom(path.clone()).expect("loads");

        runtime
            .set_keyboard_input(vec![
                RuntimeButton::Left,
                RuntimeButton::A,
                RuntimeButton::Start,
            ])
            .expect("input snapshot");
        runtime
            .set_keyboard_input(vec![RuntimeButton::Left])
            .expect("release snapshot");
        runtime.close().expect("close");

        let state = state.lock().expect("state");
        let first = state.inputs[0].1;
        assert!(first.is_pressed(Button::Left));
        assert!(first.is_pressed(Button::A));
        assert!(first.is_pressed(Button::Start));
        assert!(!first.is_pressed(Button::B));
        let second = state.inputs[1].1;
        assert!(second.is_pressed(Button::Left));
        assert!(!second.is_pressed(Button::A));
        assert!(!second.is_pressed(Button::Start));
        assert!(state.clears.contains(&InputSourceId::new(1)));
        drop(state);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn desktop_runtime_shutdown_is_idempotent_and_disconnects_requests() {
        let runtime = spawn_runtime(Arc::new(RecordingCoreFactory::default()));
        runtime.shutdown().expect("first shutdown joins");
        runtime.shutdown().expect("second shutdown is a no-op");
        assert_eq!(
            runtime.snapshot().expect_err("worker is closed").code,
            RuntimeErrorCode::RuntimeUnavailable
        );
    }

    #[test]
    fn desktop_runtime_shutdown_is_delivered_when_the_command_queue_is_full() {
        let runtime = Arc::new(spawn_runtime(Arc::new(RecordingCoreFactory::default())));
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let (entered_sender, entered_receiver) = mpsc::sync_channel(1);
        let observer = BlockingControlObserver {
            entered: entered_sender,
            release: Arc::clone(&release),
        };
        let subscribing_runtime = Arc::clone(&runtime);
        let subscribe = thread::spawn(move || subscribing_runtime.subscribe(Arc::new(observer)));
        entered_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("worker enters the blocking observer");
        runtime.test_only_fill_command_queue();

        let (shutdown_sender, shutdown_receiver) = mpsc::sync_channel(1);
        let shutting_down_runtime = Arc::clone(&runtime);
        thread::spawn(move || {
            let _ = shutdown_sender.send(shutting_down_runtime.shutdown());
        });
        assert!(
            shutdown_receiver
                .recv_timeout(Duration::from_millis(25))
                .is_err()
        );

        let (lock, condition) = &*release;
        *lock.lock().expect("release gate") = true;
        condition.notify_one();

        subscribe
            .join()
            .expect("subscription thread joins")
            .expect("subscription completes");
        shutdown_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("shutdown does not hang after queue saturation")
            .expect("shutdown succeeds");
    }

    #[test]
    fn frame_backpressure_keeps_one_in_flight_and_the_latest_pending() {
        let path = synthetic_rom_path();
        let observer = RecordingObserver::default();
        let runtime = spawn_runtime(Arc::new(ContractMockCoreFactory));
        runtime.open_rom(path.clone()).expect("loads");
        runtime
            .subscribe(Arc::new(observer.clone()))
            .expect("subscribe");
        runtime.start().expect("start");
        runtime
            .test_only_run_ticks(3)
            .expect("three deterministic ticks");

        assert_eq!(observer.frame_sequences(), vec![1]);
        runtime.acknowledge_frame(1).expect("ack first frame");
        assert_eq!(observer.frame_sequences(), vec![1, 3]);
        runtime.acknowledge_frame(3).expect("ack newest frame");
        assert_eq!(runtime.test_only_buffered_frame_count(), 0);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn pause_clears_stale_frames_before_resume() {
        let path = synthetic_rom_path();
        let observer = RecordingObserver::default();
        let runtime = spawn_runtime(Arc::new(ContractMockCoreFactory));
        runtime.open_rom(path.clone()).expect("loads");
        runtime
            .subscribe(Arc::new(observer.clone()))
            .expect("subscribe");
        runtime.start().expect("start");
        runtime.test_only_run_ticks(2).expect("buffer frames");
        assert_eq!(runtime.test_only_buffered_frame_count(), 2);

        runtime.pause().expect("pause");
        assert_eq!(runtime.test_only_buffered_frame_count(), 0);
        runtime.start().expect("resume");
        runtime.test_only_run_ticks(1).expect("fresh frame");

        assert_eq!(observer.frame_sequences(), [1, 3]);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn frame_deadline_survives_continuous_commands_and_acknowledgements() {
        let path = synthetic_rom_path();
        let runtime = spawn_runtime(Arc::new(ContractMockCoreFactory));
        let (frame_sender, frame_receiver) = mpsc::sync_channel(16);
        runtime.open_rom(path.clone()).expect("loads");
        runtime
            .subscribe(Arc::new(FrameSignalObserver {
                frames: frame_sender,
            }))
            .expect("subscribe");
        runtime.start().expect("start");

        let first = frame_receiver
            .recv_timeout(Duration::from_millis(100))
            .expect("first paced frame arrives");
        runtime.acknowledge_frame(first).expect("ack first frame");
        let traffic_deadline = Instant::now() + Duration::from_millis(75);
        let mut acknowledged = vec![first];
        while Instant::now() < traffic_deadline {
            runtime.snapshot().expect("continuous command traffic");
            while let Ok(sequence) = frame_receiver.try_recv() {
                runtime
                    .acknowledge_frame(sequence)
                    .expect("continuous frame acknowledgement");
                acknowledged.push(sequence);
            }
        }

        runtime.pause().expect("pause");
        assert!(
            acknowledged.len() >= 4,
            "absolute cadence must progress during traffic, received {acknowledged:?}"
        );
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn frame_backpressure_subscription_returns_the_published_snapshot() {
        let observer = RecordingObserver::default();
        let runtime = spawn_runtime(Arc::new(ContractMockCoreFactory));

        let snapshot = runtime
            .subscribe(Arc::new(observer.clone()))
            .expect("subscribe");

        assert_eq!(snapshot, RuntimeSnapshot::empty());
        let events = observer.control.lock().expect("control events");
        assert!(matches!(
            events.as_slice(),
            [RuntimeEvent::Snapshot { snapshot }] if snapshot == &RuntimeSnapshot::empty()
        ));
        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn frame_backpressure_rejects_stale_ack_and_clears_on_close() {
        let path = synthetic_rom_path();
        let observer = RecordingObserver::default();
        let runtime = spawn_runtime(Arc::new(ContractMockCoreFactory));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.subscribe(Arc::new(observer)).expect("subscribe");
        runtime.start().expect("start");
        runtime.test_only_run_ticks(2).expect("ticks");
        assert_eq!(
            runtime
                .acknowledge_frame(99)
                .expect_err("wrong sequence rejected")
                .code,
            RuntimeErrorCode::InvalidLifecycle
        );
        runtime.close().expect("close");
        assert_eq!(runtime.test_only_buffered_frame_count(), 0);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn frame_backpressure_replaces_observer_without_stalling_video() {
        let path = synthetic_rom_path();
        let first = RecordingObserver::default();
        let second = RecordingObserver::default();
        let runtime = spawn_runtime(Arc::new(ContractMockCoreFactory));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.subscribe(Arc::new(first.clone())).expect("first");
        runtime.start().expect("start");
        runtime.test_only_run_ticks(1).expect("first tick");
        runtime
            .subscribe(Arc::new(second.clone()))
            .expect("replace");
        runtime.test_only_run_ticks(1).expect("second tick");

        assert_eq!(first.frame_sequences(), vec![1]);
        assert_eq!(second.frame_sequences(), vec![2]);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn frame_backpressure_detaches_a_failing_observer_without_stopping_runtime() {
        let path = synthetic_rom_path();
        let observer = RecordingObserver::default();
        *observer.fail_frames.lock().expect("failure flag") = true;
        let runtime = spawn_runtime(Arc::new(ContractMockCoreFactory));
        runtime.open_rom(path.clone()).expect("loads");
        runtime.subscribe(Arc::new(observer)).expect("subscribe");
        runtime.start().expect("start");
        runtime
            .test_only_run_ticks(1)
            .expect("tick survives send failure");

        assert_eq!(
            runtime.snapshot().expect("runtime remains alive").phase,
            RuntimePhase::Running
        );
        assert_eq!(runtime.test_only_buffered_frame_count(), 0);
        runtime.shutdown().expect("shutdown");
        fs::remove_file(path).expect("remove exact synthetic ROM");
    }

    #[test]
    fn runtime_uses_the_owned_video_queue_without_local_duplicates() {
        let source = include_str!("runtime.rs");
        assert!(source.contains("FrameQueue"));
        assert!(!source.contains(concat!("struct Frame", "Delivery")));
        assert!(!source.contains(concat!("fn encode_frame", "_packet")));
    }
}
