//! Narrow support boundary for cross-crate integration tests.

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;

use gb_core::InputSourceId;

pub use crate::emulator::contracts::{RuntimeButton, RuntimePhase, RuntimeResult, RuntimeSnapshot};
pub use crate::emulator::runtime::{CoreFactory, RuntimeCore};

use crate::audio::{AudioBackendError, AudioBackendErrorKind, AudioOutput, AudioOutputFactory};
use crate::emulator::runtime::DesktopRuntime;
use crate::remote::contracts as remote_contracts;
use crate::remote::manager::RemoteSessionManager;

/// Keyboard input source used by the desktop runtime.
pub const KEYBOARD_INPUT_SOURCE: InputSourceId = crate::emulator::contracts::KEYBOARD_INPUT_SOURCE;
/// Remote controller input source used by the desktop runtime.
pub const REMOTE_INPUT_SOURCE: InputSourceId = crate::emulator::contracts::REMOTE_INPUT_SOURCE;

/// Observable remote-session lifecycle exposed only to integration tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemotePhase {
    Off,
    Waiting,
    Connected,
    Error,
}

/// Bounded latency summary exposed only to integration tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteLatency {
    pub samples: u64,
    pub p95_ms: u64,
}

/// Observable remote-session state exposed only to integration tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSnapshot {
    pub phase: RemotePhase,
    pub pairing_url: Option<String>,
    pub latency: Option<RemoteLatency>,
}

/// Result returned by the narrow integration-test support boundary.
pub type RemoteResult<T> = Result<T, String>;

impl From<remote_contracts::RemotePhase> for RemotePhase {
    fn from(phase: remote_contracts::RemotePhase) -> Self {
        match phase {
            remote_contracts::RemotePhase::Off => Self::Off,
            remote_contracts::RemotePhase::Waiting => Self::Waiting,
            remote_contracts::RemotePhase::Connected => Self::Connected,
            remote_contracts::RemotePhase::Error => Self::Error,
        }
    }
}

fn remote_snapshot(snapshot: remote_contracts::RemoteSnapshot) -> RemoteSnapshot {
    RemoteSnapshot {
        phase: snapshot.phase.into(),
        pairing_url: snapshot.pairing_url,
        latency: snapshot.latency.map(|latency| RemoteLatency {
            samples: latency.samples,
            p95_ms: latency.p95_ms,
        }),
    }
}

fn remote_error(error: remote_contracts::RemoteError) -> String {
    let remote_contracts::RemoteError { code, message } = error;
    format!("{code:?}: {message}")
}

struct UnavailableAudioFactory;

impl AudioOutputFactory for UnavailableAudioFactory {
    fn open_default(&self) -> Result<Box<dyn AudioOutput>, AudioBackendError> {
        Err(unavailable_audio())
    }
}

fn unavailable_audio() -> AudioBackendError {
    AudioBackendError::new(
        AudioBackendErrorKind::NoOutputDevice,
        "audio is disabled for remote runtime integration tests",
    )
}

/// Owns a real desktop runtime and loopback remote-session manager for integration tests.
pub struct RemoteRuntimeHarness {
    runtime: DesktopRuntime,
    remote: RemoteSessionManager,
}

impl RemoteRuntimeHarness {
    #[must_use]
    pub fn new(factory: Arc<dyn CoreFactory>, controller_assets: PathBuf) -> Self {
        let sample_rate = NonZeroU32::new(48_000).expect("48 kHz is non-zero");
        let runtime = DesktopRuntime::spawn_with_audio_preflight(
            factory,
            Arc::new(UnavailableAudioFactory),
            sample_rate,
            Err(unavailable_audio()),
        );
        let remote = RemoteSessionManager::new_loopback(runtime.handle(), controller_assets);
        Self { runtime, remote }
    }

    /// Loads a ROM through the real desktop runtime worker.
    ///
    /// # Errors
    ///
    /// Returns the runtime error reported by the worker.
    pub fn open_rom(&self, path: PathBuf) -> RuntimeResult<RuntimeSnapshot> {
        self.runtime.open_rom(path)
    }

    /// Starts the real desktop runtime worker.
    ///
    /// # Errors
    ///
    /// Returns the runtime error reported by the worker.
    pub fn start_runtime(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.runtime.start()
    }

    /// Pauses the real desktop runtime worker.
    ///
    /// # Errors
    ///
    /// Returns the runtime error reported by the worker.
    pub fn pause_runtime(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.runtime.pause()
    }

    /// Restarts the loaded ROM through the real desktop runtime worker.
    ///
    /// # Errors
    ///
    /// Returns the runtime error reported by the worker.
    pub fn restart_runtime(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.runtime.restart()
    }

    /// Reports the real desktop runtime snapshot.
    ///
    /// # Errors
    ///
    /// Returns the runtime error reported by the worker.
    pub fn runtime_snapshot(&self) -> RuntimeResult<RuntimeSnapshot> {
        self.runtime.snapshot()
    }

    /// Replaces the keyboard input snapshot in the real desktop runtime.
    ///
    /// # Errors
    ///
    /// Returns the runtime error reported by the worker.
    pub fn set_keyboard_input(&self, buttons: Vec<RuntimeButton>) -> RuntimeResult<()> {
        self.runtime.set_keyboard_input(buttons)
    }

    /// Stops the real desktop runtime worker.
    ///
    /// # Errors
    ///
    /// Returns the runtime error reported by the worker.
    pub fn shutdown_runtime(&self) -> RuntimeResult<()> {
        self.runtime.shutdown()
    }

    pub fn start_remote(&self) -> RemoteResult<RemoteSnapshot> {
        self.remote
            .start()
            .map(remote_snapshot)
            .map_err(remote_error)
    }

    pub fn end_remote(&self) -> RemoteResult<RemoteSnapshot> {
        self.remote.end().map(remote_snapshot).map_err(remote_error)
    }

    pub fn remote_snapshot(&self) -> RemoteResult<RemoteSnapshot> {
        self.remote
            .snapshot()
            .map(remote_snapshot)
            .map_err(remote_error)
    }
}

impl Drop for RemoteRuntimeHarness {
    fn drop(&mut self) {
        let _ = self.remote.shutdown();
        let _ = self.runtime.shutdown();
    }
}
