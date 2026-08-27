# Core Contracts

`gb-core` currently defines contracts only. It has no dependencies beyond the Rust standard library and implements no CPU instructions, PPU, APU, ROM loading, persistence I/O, or platform integration.

## Emulator boundary

`EmulatorCore` exposes the following frozen methods:

```rust
fn load_rom(&mut self, rom: &[u8], persisted: Option<&BatteryState>)
    -> Result<CartridgeMetadata, CoreError>;
fn reset(&mut self) -> Result<(), CoreError>;
fn run_cycles(&mut self, cycle_budget: u32) -> Result<RunOutcome, CoreError>;
fn set_input(&mut self, source: InputSourceId, state: JoypadState);
fn clear_input_source(&mut self, source: InputSourceId);
fn take_frame(&mut self) -> Option<Frame>;
fn drain_audio(&mut self) -> AudioBatch;
fn battery_state(&self) -> Option<BatteryState>;
```

The future runtime serializes load, reset, run, input, and close-related operations outside the core. `run_cycles` returns executed cycles plus frame/audio availability; it does not pace wall-clock time.

## Data contracts

- `Frame` owns an immutable RGBA buffer of exactly `SCREEN_WIDTH * SCREEN_HEIGHT * 4` bytes. `SCREEN_WIDTH` is `160`, `SCREEN_HEIGHT` is `144`, and `sequence` is monotonic for produced frames.
- `AudioBatch` owns interleaved `f32` stereo pairs and a non-zero sample rate. The runtime drains batches into a bounded platform queue; the core boundary must never imply an unbounded consumer backlog.
- `Button` contains only Up, Down, Left, Right, A, B, Start, and Select. `JoypadState` uses one internal bit per button and can form a union without representing invalid buttons.
- `InputSourceId` is an opaque `u64` newtype. The runtime maintains a `JoypadState` per source and unions them. Clearing a disconnected source must preserve every other source.
- `BatteryState` carries a format version, RAM bytes, and mapper-private bytes. It contains data only; atomic files, application directories, checkpoints, and corrupt-save preservation belong to the platform runtime.
- `Clock::unix_seconds` injects external time so future MBC3 behavior and tests remain deterministic.
- `CartridgeMetadata` identifies title, stable ROM identity, mapper, DMG compatibility mode, RAM size, and battery capability.
- `CoreError` distinguishes invalid ROM, CGB-only cartridge, unsupported mapper, not-loaded state, and internal invariant failure.

`MapperKind` currently names ROM-only, MBC1, MBC3, and MBC5 as contract values; their behavior is not implemented by PED-34.
