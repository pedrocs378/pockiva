# Runtime Boundaries

The Tauri crate is the platform adapter. Production registers the ROM lifecycle, keyboard input, binary video delivery, audio output, and remote-session commands. It negotiates the default audio sample rate, constructs `GameBoy<SystemClock>` through `GameBoyCoreFactory`, and keeps the emulator worker independent from Tauri command handlers.

## Ownership at runtime

- The platform runtime owns file dialogs, the emulation thread, lifecycle and input command serialization, wall-clock pacing, audio devices, framebuffer presentation, and remote-session start/stop.
- `gb-core` consumes ROM bytes, persisted bytes, source-aware input snapshots, clock values, and cycle budgets. It never opens files, sockets, windows, or audio devices.
- `gb-network` validates JSON messages and owns pairing/session/connection state. It emits authenticated `ControllerEvent` values; it never calls emulator internals directly.
- React applications render state and issue typed adapter requests. The browser controller imports only `@gameboy/protocol`, never desktop source.

Framebuffer transfer must use reusable binary storage or a bounded producer/consumer queue. Per-frame PNG or base64 conversion is outside the accepted boundary. Audio delivery must also be bounded; a slow device cannot cause unbounded memory growth.

Keyboard input is reserved as `InputSourceId::new(1)` and the single remote controller as `InputSourceId::new(2)`. Both enter the same bounded emulator-worker queue, and only `gb-core::InputMatrix` computes their effective union. Remote input survives pause, resume, restart, and ROM replacement. WebSocket close, heartbeat timeout, or explicit session end clears only source `2`; source `1`, the loaded ROM, video, and audio remain intact. On application exit, this remote cleanup occurs first and the normal emulator shutdown then releases the remaining runtime resources.

Battery/mapper bytes cross the core boundary through `BatteryState`. The runtime chooses the stable ROM identity path and performs atomic writes on clean close, ROM replacement, shutdown, and later dirty checkpoints. Corrupt files are preserved for diagnosis rather than overwritten.

The local server remains disabled until the user starts a session. It discovers and binds one non-loopback LAN IPv4 address on an ephemeral port, serves only the bundled `controller` build directory, and upgrades WebSockets only at `/controller`. The pairing URL contains a cryptographically random 32-byte token with a ten-minute authentication lifetime; tokens are redacted from debug output. Requests with a mismatched `Origin`, invalid or expired token, unsupported protocol, non-contiguous sequence, oversized frame, malformed message, or excess rate are rejected. One authenticated controller owns the session at a time, with a production token-bucket rate of 240 messages per second and a burst capacity of 64.

The remote manager converts network `Connected`, `Message`, and `Disconnected` events into source `2` runtime commands and publishes ordered snapshots to React. Every authenticated connection clears source `2` exactly once when it disconnects. Session shutdown is synchronous: the listener invalidates its token, closes and joins socket tasks, completes that remote cleanup, and only then may the emulator runtime stop. Static-file resolution rejects symlinks and special files and cannot escape the bundled controller asset root. No network path exposes ROM bytes, emulator internals, or arbitrary filesystem data.
