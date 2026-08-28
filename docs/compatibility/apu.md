# DMG APU and Desktop Audio Compatibility

PED-49 implements deterministic original-DMG audio in `gb-core` and a bounded CPAL output adapter in the desktop application. The core remains platform independent. Dual-mode cartridges continue to run in DMG mode; this document does not claim CGB audio behavior.

## Implemented boundary

- The core implements pulse channels 1 and 2, channel-one sweep, envelopes and length counters, the programmable wave channel and active wave-RAM behavior, the 15-bit/7-bit noise LFSR, NR50/NR51 routing, DAC power behavior, deterministic high-pass filtering, and master power/reset semantics.
- The frame sequencer observes natural and `FF04`-caused falling edges of the post-boot divider mirror. `MachineBus` advances the APU once per T-cycle without changing the frozen `AudioDevice` or public `EmulatorCore` contracts.
- Stereo PCM uses integer-rate accumulation, pair-safe bounded buffering, deterministic draining, and exact negotiated sample-rate tags.
- The desktop adapter uses CPAL `0.18.2` with platform defaults disabled and ringbuf `0.5.1`. Its callback queue is bounded to 160 ms with 40/80/120 ms low/target/high watermarks.
- Runtime pacing is audio-primary when a healthy output exists and falls back to the monotonic video deadline when output is absent or becomes unusable. Every core batch is drained, including fallback operation. Pause, restart, replacement, close, fatal-core failure, stream failure, and shutdown explicitly flush or release owned audio state.

## Automated evidence

| Boundary | Evidence | Result |
| --- | --- | --- |
| APU channel/timing/mixer/PCM matrix | 23 isolated integration vectors, including envelope saturation/direction, all pulse duties, wave access/retrigger, both noise widths, all NR50/NR51 routes, filter reset, and irregular tick/drain partitioning | Passed |
| Bus integration | Natural and divider-reset frame-sequencer edges, four APU T-cycles per machine cycle, reset reconstruction, and bounded draining | Passed |
| Desktop queue/backend policy | Queue capacity, underrun/flush/error behavior, rate rejection, watermarks, pacing decisions, CPAL config/error mapping | Passed; hardware smoke remains optional and ignored |
| Desktop runtime | Negotiated 44.1 kHz batches, 48 kHz fallback, video/input continuity, priming, high-water backpressure, exact lifecycle ordering, repeated lifecycle calls, replacement, stale-frame clearing, fatal-core and terminal-stream fallback | Passed |
| Complete Rust workspace | 188 tests passed; 25 ignored | Passed |
| Complete desktop Rust package after lifecycle review | 54 tests passed; 2 ignored | Passed |
| JavaScript workspace | protocol 7, remote controller 48, desktop 112; lint, typecheck, tests, and production builds | Passed |
| Dependency isolation | `gb-core` contains no CPAL/ringbuf/platform dependency; desktop resolves CPAL `0.18.2` and ringbuf `0.5.1` without ASIO/JACK/PipeWire/PulseAudio features | Passed |
| macOS Apple Silicon compile check | `aarch64-apple-darwin`, all features | Passed |

The ignored deterministic runtime soak is configured for 30 samples over 30 minutes, ten pause/resume cycles, five restarts, three replacements, bounded occupancy, and progress checks. It was compiled but not executed in this checkpoint. The optional real-device synthetic-tone smoke was also not run.

## Open acceptance gates

- Windows x64 MSVC compile and test evidence requires a Windows runner or an installed Rust MSVC target. This macOS Homebrew toolchain has no `rustup`; the target check therefore stopped before compiling project code.
- The checksum-gated Blargg `dmg_sound.gb` asset is absent. Expected SHA-256: `c34e740664eb14b42c39750434e3e105fc92d774a98fb671594a48e972401630`. The ignored harness is compiled and discoverable, but PED-49 compatibility acceptance remains blocked until a user-provisioned matching file passes all twelve cases.
- Packaged real-ROM CoreAudio/WASAPI listening and manual application lifecycle are deferred to PED-40, as planned.

No commercial ROM was downloaded, stored, or used.
