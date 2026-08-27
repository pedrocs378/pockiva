# Game Boy Desktop Emulator Design

**Issue:** PED-32  
**Date:** 2026-08-27  
**Targets:** macOS Apple Silicon and Windows x64

## Objective

Build a desktop Game Boy emulator that is fully playable with a keyboard and can optionally use one mobile browser as a controller over the local network. Connecting or disconnecting the mobile controller must not restart the loaded ROM. The emulator must keep UI, transport, platform runtime, and emulation concerns behind explicit contracts.

## MVP Scope

The MVP must:

- open local Game Boy ROMs;
- emulate the CPU, memory map, cartridge, timers, interrupts, input, PPU, and APU required by the compatibility target;
- render a stable `160 x 144` image while preserving aspect ratio;
- produce acceptably synchronized stereo audio;
- start, pause, restart, and close a ROM;
- support configurable keyboard mappings for D-Pad, A, B, Start, and Select;
- persist battery-backed cartridge RAM between application sessions;
- optionally start a local controller session and show a QR Code;
- provide a responsive mobile web controller with multi-touch input;
- allow the mobile controller to connect, disconnect, and reconnect while a ROM continues to run;
- run original Game Boy cartridges and dual-mode DMG/GBC cartridges in DMG compatibility mode;
- reject Game Boy Color-only cartridges with a clear error.

Full Game Boy Color hardware support, save states, fast-forward, rewind, cheats, link cable, physical gamepads, cover libraries, and native mobile applications remain outside the MVP.

## Compatibility Target

Hardware behavior will target the original Game Boy DMG. Initial cartridge support includes ROM-only cartridges and MBC1, MBC3, and MBC5, including external RAM and battery-backed saves where applicable. RTC support for MBC3 must be represented behind a clock abstraction so tests remain deterministic and persisted RTC data can evolve without coupling the core to the operating system.

Dual-mode cartridges may execute through their DMG-compatible path. CGB-only cartridges must fail during cartridge validation before emulation begins. Dual-mode games run with the emulator's selected monochrome DMG palette; CGB palettes and CGB-only rendering features are not emulated.

Automated compatibility validation uses Blargg and Mooneye test ROMs. End-to-end validation also uses a legally redistributable homebrew ROM. Commercial ROMs are neither bundled nor downloaded by the application.

## Repository Structure

```text
apps/
  desktop/             # React desktop UI and Tauri runtime
  remote-controller/   # Responsive React mobile web client

crates/
  gb-core/             # Platform-independent emulation core
  gb-network/          # Local HTTP/WebSocket session and pairing

packages/
  protocol/            # Versioned TypeScript protocol definitions
```

The repository is a pnpm workspace for JavaScript and TypeScript packages and a Cargo workspace for Rust crates. Project-wide commands must make formatting, linting, type checking, tests, and builds repeatable from the repository root.

## Component Responsibilities

### `crates/gb-core`

`gb-core` owns CPU state and instruction execution, the memory bus, cartridge controllers, timers, interrupts, joypad registers, PPU, APU, and deterministic emulation timing. It must not depend on React, Tauri, network transports, filesystem APIs, or platform audio APIs.

The core consumes ROM bytes, input state, persisted cartridge state, and clock values supplied through contracts. It produces framebuffer data, audio samples, cartridge persistence data, and structured faults.

### `apps/desktop`

The desktop application owns the React user interface and the Tauri platform adapter. The Rust runtime opens ROMs, validates them through `gb-core`, manages an emulation thread, communicates lifecycle commands, persists application settings and battery RAM, exposes framebuffer/audio output to the UI and platform devices, and starts or stops remote-control sessions.

High-frequency video and audio paths must avoid per-frame PNG/base64 encoding. The concrete bridge may use reusable binary buffers or a native-side producer/consumer queue, but it must preserve the core boundary and apply bounded buffering so slow consumers cannot grow memory indefinitely.

### `crates/gb-network`

`gb-network` owns the local HTTP/WebSocket server, session lifetime, short-lived pairing token, message validation, connection state, and disconnect cleanup. It emits protocol-level controller events and does not mutate emulator internals directly.

### `apps/remote-controller`

The mobile client owns the touch interface, connection lifecycle, protocol serialization, and visual connection/input feedback. It runs in a mobile browser without requiring native installation. PWA metadata and installability may be provided, but offline installation is not a completion requirement.

### `packages/protocol`

The protocol package is the canonical TypeScript representation of protocol version `v1`. Rust mirrors the wire schema with serialization tests and checked fixtures so incompatible changes are detected. Messages cover handshake, connection acceptance/rejection, button down, button up, state synchronization, ping/pong, and disconnect.

## Core Contracts

The detailed Rust types are finalized in PED-34, but the public boundary must represent these concepts:

- `Cartridge::load(bytes)` validates the header, compatibility mode, mapper, ROM size, and RAM size;
- `GameBoy::step()` executes one instruction and advances all clocked subsystems by the resulting machine cycles;
- input is supplied by source and merged into one joypad state;
- a completed frame is exposed as a `160 x 144` pixel buffer through a non-platform-specific format;
- generated stereo PCM samples are drained in bounded batches;
- battery RAM and mapper persistence can be loaded and extracted as bytes plus metadata;
- external clock access is injectable for deterministic MBC3 behavior;
- faults distinguish invalid ROMs, unsupported cartridges, illegal states, and platform/runtime errors.

The desktop runtime communicates with the emulation thread using explicit lifecycle and input commands. Start, pause, restart, close, and input updates are serialized through the command channel. Closing a ROM must flush battery state, release audio/video buffers, clear every input source, and leave the runtime ready to load another ROM.

## Input Model

Keyboard and remote inputs are tracked as separate sources. The effective joypad state is the union of all active sources, which allows keyboard and mobile input to coexist. Releasing or disconnecting one source clears only that source's pressed buttons. A heartbeat timeout and WebSocket close both clear the remote source, preventing stuck buttons after an unexpected disconnect.

Default keyboard mappings are provided and persisted in application settings. Remapping must reject ambiguous or reserved bindings where they would make the desktop controls unusable. Key events are ignored while the user is editing a text/control field.

## Runtime and Data Flow

```text
local ROM -> Tauri runtime -> gb-core -> framebuffer -> desktop viewport
                            |        -> PCM samples -> platform audio
                            |        -> battery RAM -> application data
keyboard ------------------+-> source-aware InputState
mobile -> WebSocket -> gb-network -----^
```

The emulation loop uses the Game Boy clock as the source of simulated time and paces wall-clock output at the runtime boundary. Audio buffering is the primary pacing signal when audio is enabled; a monotonic clock provides fallback pacing. Pause suspends core advancement and audio production without destroying the loaded cartridge.

## Desktop Experience

The main desktop screen contains a centered game viewport with nearest-neighbor scaling, ROM selection, lifecycle controls, current emulation status, remote-controller status, keyboard mapping settings, and a connection panel. Empty, loading, invalid-ROM, unsupported-cartridge, runtime-error, paused, and running states must be explicit.

The remote panel can start a session, display its QR Code and local URL, show whether a controller is connected, and end the session. Ending the session clears remote input but leaves keyboard play and the running ROM intact.

## Mobile Experience

The controller places a D-Pad on the left, A/B on the right, and Start/Select centrally. It supports simultaneous touches, portrait and landscape layouts, and safe-area insets. During gameplay it prevents accidental scrolling, selection, context menus, and zoom gestures. Visual state reflects local touch immediately while connection state distinguishes connecting, connected, disconnected, expired token, incompatible protocol, and server unavailable.

The MVP permits one connected mobile controller. A second client receives a clear rejection while the active controller remains connected.

## Local Network Security

The controller server binds only to interfaces needed for the advertised local address and is disabled until the user starts a session. Each session uses a cryptographically random, unguessable token embedded in the QR Code URL. The token is invalidated when the session ends or the application exits.

The server validates protocol version, message shape, button names, message size, and connection state. It rate-limits abusive input, accepts only the expected WebSocket route, and does not expose filesystem or ROM contents. Pairing is intentionally local-network-only; no cloud relay or account system is introduced.

## Persistence

Application settings and saves live under the platform application-data directory, not beside the original ROM. A stable ROM identity derived from cartridge metadata and content identifies save data without exposing filenames to the mobile client. Save writes are atomic and occur on clean close, ROM replacement, application shutdown, and periodic dirty checkpoints. Corrupt or incompatible save data produces a recoverable error and preserves the original file for diagnosis.

## Error Handling

Errors are typed at subsystem boundaries and translated into user-facing messages by the desktop UI. Unsupported CGB-only ROMs, unsupported mappers, malformed headers, inaccessible files, audio-device failures, save failures, and network binding failures must be distinguishable. Audio and remote-control failures should degrade gracefully when possible: keyboard gameplay remains available if a controller session fails, and video/input remain usable if audio initialization fails after informing the user.

## Testing Strategy

### Rust tests

- unit tests cover registers, flags, instruction behavior, memory mapping, timers, interrupts, joypad behavior, mappers, PPU timing/rendering primitives, and APU registers/timing;
- deterministic fixtures cover protocol mirroring and cartridge header validation;
- a ROM harness executes selected Blargg and Mooneye ROMs with bounded cycle counts and explicit pass/fail signals;
- integration tests cover lifecycle, persistence, input-source merging, disconnect cleanup, framebuffer production, and bounded audio output.

### TypeScript and UI tests

- protocol encoding and validation tests use shared fixtures;
- desktop tests cover lifecycle states, keyboard mapping, error presentation, and remote-session UI;
- mobile tests cover multi-touch, button release, connection states, orientation-sensitive layout behavior, and prevention of accidental browser gestures.

### Platform and end-to-end validation

- production builds are verified for macOS Apple Silicon and Windows x64;
- a redistributable homebrew ROM validates load-to-gameplay behavior;
- manual checks cover pause, restart, close, save/reload, keyboard-only play, QR pairing, controller disconnect/reconnect, and switching between keyboard and mobile without restarting the ROM;
- a prolonged run checks resource stability and bounded queues;
- known compatibility limitations and the exact test ROM revisions are documented.

Tests may download redistributable test assets through an explicit developer script, but ROM binaries must not be silently fetched during ordinary application use and must not be committed unless their licenses permit redistribution.

## Delivery Sequence and Linear Synchronization

PED-32 remains the orchestrator. Work follows the dependency graph recorded in Linear:

1. PED-34 establishes the workspace, architectural boundaries, contracts, and test foundations.
2. After PED-34, PED-35, PED-37, and PED-38 may proceed in parallel with exclusive module ownership.
3. After PED-35, PED-36 and PED-49 may proceed in parallel.
4. After PED-35, PED-37, and PED-38, PED-39 integrates the controller path.
5. PED-40 performs final compatibility, stability, and end-to-end validation after its dependencies are complete.

Immediately before work begins on a sub-issue, its Linear status moves to **In Progress**. It moves to **In Review** only when implementation and local validation are ready for review, and to **Done** only after its acceptance criteria pass. Blocked issues remain in Backlog/Todo. PED-32 moves to **In Progress** when PED-34 begins and reaches **Done** only after every required sub-issue, including PED-40, is Done.

## Completion Criteria

The MVP is complete when:

- a supported DMG or dual-mode cartridge opens and reaches gameplay;
- CPU, memory, timers, interrupts, PPU, APU, and input meet the recorded compatibility baseline;
- video and audio remain stable for the tested scope;
- configurable keyboard input provides complete gameplay without a phone;
- battery-backed progress survives application restarts;
- one mobile controller pairs by QR Code over the local network and can connect, disconnect, and reconnect without restarting the ROM;
- an unexpected controller disconnect cannot leave buttons pressed;
- `gb-core` remains free of React, Tauri, filesystem, network, and platform-audio dependencies;
- lint, formatting, type checking, automated tests, platform builds, and documented manual checks pass;
- all mandatory Linear sub-issues are Done and PED-40 records the final validation.

