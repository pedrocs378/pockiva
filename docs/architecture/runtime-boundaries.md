# Runtime Boundaries

The Tauri crate is the future platform adapter. In PED-34 it registers only `foundation_status`, which reports protocol `v1`, the `160 × 144` screen contract, and a one-controller limit. It does not load a ROM or start a network server.

## Ownership at runtime

- The platform runtime will own file dialogs, application-data paths, atomic save writes, the emulation thread, lifecycle command serialization, wall-clock pacing, audio devices, framebuffer presentation, and remote-session start/stop.
- `gb-core` will consume ROM bytes, persisted bytes, input snapshots, clock values, and cycle budgets. It will never open files, sockets, windows, or audio devices.
- `gb-network` will validate JSON messages and own pairing/session/connection state. It emits `ControllerEvent` values; it never calls emulator internals directly.
- React applications render state and issue typed adapter requests. The browser controller imports only `@gameboy/protocol`, never desktop source.

Framebuffer transfer must use reusable binary storage or a bounded producer/consumer queue. Per-frame PNG or base64 conversion is outside the accepted boundary. Audio delivery must also be bounded; a slow device cannot cause unbounded memory growth.

Keyboard and controller inputs use separate `InputSourceId` values. The effective joypad value is their union. WebSocket close, heartbeat timeout, or explicit session end clears only the remote source, so keyboard play and a loaded ROM remain intact.

Battery/mapper bytes cross the core boundary through `BatteryState`. The runtime chooses the stable ROM identity path and performs atomic writes on clean close, ROM replacement, shutdown, and later dirty checkpoints. Corrupt files are preserved for diagnosis rather than overwritten.

The future local server remains disabled until requested, binds only required local interfaces, accepts only its controller route, limits one controller, validates version/message size/state/rate, and uses a random short-lived token. It exposes neither ROM bytes nor filesystem paths.
