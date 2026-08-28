# PED-39 Remote Controller Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect one real mobile browser to a real running `GameBoy` through an authenticated local HTTP/WebSocket session, while keyboard input, ROM lifecycle, video, and audio continue independently.

**Architecture:** `gb-network` owns a small Axum server and its protocol-v1 session state, validates every frame, and emits source-aware `ControllerEvent` values through a narrow sink. The desktop adapter owns the platform resource path, the real `SystemClock + GameBoy` factory, serialization of remote input onto the existing emulator worker, session presentation, and shutdown order. The mobile application remains an independent protocol-v1 client; its built static assets are bundled as a Tauri resource and served locally without importing desktop source.

**Tech Stack:** Rust stable 2024, `gb-core`, `gb-network`, Axum 0.8.4, Tokio 1.53.1, Tower HTTP 0.6.11, Tauri 2.11.5, React 19.2.8, TypeScript 7.0.2, `qrcode.react` 4.2.0, Vitest 4.1.11, pnpm 11.24.0.

**Spec:** `docs/superpowers/specs/2026-08-27-game-boy-emulator-design.md`

## Global Constraints

- PED-35, PED-37, and PED-38 must remain `Done`; re-fetch them and PED-39 in Linear immediately before implementation.
- Do not start PED-39 runtime edits until the coordinator has completed PED-36 video integration, PED-49 APU/T-cycle integration, and PED-49 desktop audio integration in that total order.
- Preserve `CoreFactory::create(&self) -> Box<dyn RuntimeCore>` exactly. The negotiated audio sample rate is captured in the production factory; it is never added as a `create` argument.
- Production startup must use `GameBoy<SystemClock>`; `ContractMockCoreFactory` remains test-only after this issue.
- Reserve `InputSourceId::new(1)` for keyboard and `InputSourceId::new(2)` for the single remote controller. Never merge those snapshots in desktop code; `gb-core::InputMatrix` owns the union.
- Preserve protocol `v1` byte-for-byte. Do not modify `packages/protocol/src/messages.ts`, `packages/protocol/fixtures/protocol-v1.json`, or their public unions.
- The only WebSocket route is `/controller`. Static HTTP requests may resolve only files under the configured controller asset root, with `index.html` as the directory fallback.
- The server is off until the user starts a session. Production binds the selected non-loopback LAN address, never `0.0.0.0`; tests may inject `127.0.0.1`.
- Generate 32 random token bytes using the operating system RNG and encode URL-safe without padding. Tokens expire ten minutes after session start and become invalid immediately on session end or application exit.
- Allow at most one authenticated controller. A second connection receives `controller-already-connected` without disturbing the active controller.
- Limit WebSocket text messages to `4_096` UTF-8 bytes, apply a token bucket refilled at `240` sequenced messages/second with capacity `64`, and reject binary/unknown/out-of-order messages as `malformed-message`.
- The server heartbeat deadline is 18 seconds since the most recent valid client message. WebSocket close, heartbeat timeout, session end, and application exit emit exactly one remote-source cleanup.
- A remote-session failure must not stop the ROM, keyboard, video, or audio. Ending a session clears source `2` and leaves the emulation phase unchanged.
- The mobile client continues to derive `ws(s)://<same-host>:<same-port>/controller` from `http(s)://<host>:<port>/?token=<token>`, sends `hello` first, sends full `state-sync` after every `welcome`, and retains its existing heartbeat/reconnect behavior.
- All JavaScript dependencies are exact versions. Cargo manifests and both lockfiles are shared coordinator-owned files; reconcile them serially and commit each lockfile without unrelated changes.
- Do not commit or download commercial ROMs. Real-ROM acceptance uses an already provisioned redistributable ROM.
- Every local shell command in this plan is prefixed with `rtk`.

## Ownership and Total Order

| Owner | Exclusive paths during PED-39 | Shared paths entered only at the named serial gate |
| --- | --- | --- |
| Network owner | `crates/gb-network/src/server.rs`, `crates/gb-network/src/rate_limit.rs`, `crates/gb-network/tests/live_session.rs` | `crates/gb-network/src/lib.rs`, `session.rs`, `Cargo.toml` |
| Runtime owner | `apps/desktop/src-tauri/src/remote/**`, `apps/desktop/src-tauri/src/emulator/factory.rs` | `apps/desktop/src-tauri/src/emulator/runtime.rs`, `emulator/mod.rs`, `lib.rs`, `build.rs`, `Cargo.toml`, `tauri.conf.json`, permissions/capabilities |
| Desktop UI owner | `apps/desktop/src/features/remote-controller/**` | `apps/desktop/src/pages/EmulatorPage.tsx`, tests, `styles.css`, `package.json` |
| Mobile correction owner | only regression tests/fixes under `apps/remote-controller/src/features/session/**` | `apps/remote-controller/package.json`, Vite config only if a proven production integration defect requires it |
| Coordinator | Linear transitions, shared manifests, `Cargo.lock`, `pnpm-lock.yaml`, cross-lane runtime merge | all shared paths after PED-36/PED-49 release them |

The mandatory execution order is Tasks 0-3, then Task 4, then Task 5, then Tasks 6-9, then Tasks 10-12. Network pure-state and desktop UI work may be delegated in parallel only after Task 1 commits the manifests and locks; no worker may edit `runtime.rs`, `lib.rs`, a manifest, or a lockfile concurrently. Task 5 receives `runtime.rs` only after the PED-49 runtime/audio integration is green and owns the sole remote-input change there.

## Contract Map

| File | Responsibility |
| --- | --- |
| `crates/gb-network/src/session.rs` | Pure authenticated connection/session machine and protocol-v1 sequencing; no socket or emulator mutation. |
| `crates/gb-network/src/rate_limit.rs` | Deterministic rolling-window limiter accepting injected `Instant`. |
| `crates/gb-network/src/server.rs` | Tokio/Axum listener, static controller assets, `/controller` upgrade, deadlines, shutdown, and event emission. |
| `apps/desktop/src-tauri/src/emulator/factory.rs` | `SystemClock` and zero-argument `GameBoyCoreFactory::create`. |
| `apps/desktop/src-tauri/src/emulator/runtime.rs` | Serialize `ControllerEvent` on the existing worker and retain one complete remote snapshot per source. |
| `apps/desktop/src-tauri/src/remote/contracts.rs` | Tauri-serializable remote snapshot/error/event contract. |
| `apps/desktop/src-tauri/src/remote/manager.rs` | Start/end one server, bridge events to the runtime handle, observe latency, and publish UI snapshots. |
| `apps/desktop/src-tauri/src/remote/commands.rs` | Thin Tauri command adapters only. |
| `apps/desktop/src/features/remote-controller/*` | Parse native snapshots, subscribe, start/end sessions, render URL and QR. |

---

### Task 0: Revalidate dependencies, ownership, and native gates

**Files:**
- Read: Linear issues `PED-35`, `PED-37`, `PED-38`, `PED-39`.
- Read: `docs/superpowers/plans/2026-08-27-ped-32-orchestration.md`.
- Read: merged `apps/desktop/src-tauri/src/emulator/runtime.rs` and `apps/desktop/src-tauri/src/audio/mod.rs`.
- Modify: no repository files.

**Interfaces:**
- Consumes: completed PED-35/37/38 and coordinator-completed PED-36/PED-49 serial integration.
- Produces: an explicit go/no-go record and the exact post-audio `DesktopRuntime::spawn(...)` signature used by every later task.

- [ ] **Step 1: Re-fetch the Linear blockers and current issue**

Use the Linear connector to fetch all four issues with relations. Record that PED-35, PED-37, and PED-38 are `Done`; leave PED-39 out of `In Progress` if any blocker regressed.

- [ ] **Step 2: Verify the shared runtime handoff**

```bash
rtk git log --oneline -20
rtk rg -n "pub trait CoreFactory|pub fn spawn|runtime_sample_rate|AudioOutputFactory|CpalAudioOutputFactory" apps/desktop/src-tauri/src/emulator/runtime.rs apps/desktop/src-tauri/src/audio/mod.rs apps/desktop/src-tauri/src/lib.rs
rtk cargo test -p gameboy-desktop emulator::runtime --all-features
```

Expected: the merged tree contains PPU and APU integration, audio-primary pacing, unchanged zero-argument `CoreFactory::create`, and `DesktopRuntime::spawn(factory, audio_factory, runtime_sample_rate, prepared_output)`. Stop for coordinator reconciliation if the merged constructor differs; never improvise a second constructor or change `CoreFactory::create` to compensate.

- [ ] **Step 3: Verify test/build environments before dispatch**

```bash
rtk rustup target list --installed
rtk pnpm --filter @gameboy/remote-controller build
rtk cargo check -p gameboy-desktop --target aarch64-apple-darwin
```

Confirm access to a Windows x64 runner with Rust stable and Node.js 24.20.0. Missing Windows hardware does not block writing code, but it blocks Task 12 and therefore blocks moving PED-39 to `Done`.

- [ ] **Step 4: Move only PED-39 to `In Progress`**

Use the Linear connector only after Steps 1-3 pass. Do not change PED-40.

### Task 1: Add exact server, QR, and packaged-asset dependencies serially

**Files:**
- Modify: `crates/gb-network/Cargo.toml`.
- Modify: `apps/desktop/package.json`.
- Modify: `apps/desktop/src-tauri/tauri.conf.json`.
- Modify: `Cargo.lock`.
- Modify: `pnpm-lock.yaml`.

**Interfaces:**
- Consumes: Axum 0.8.4 APIs verified through Context7: `Router::route`, `WebSocketUpgrade::on_upgrade`, `axum::serve`, `into_make_service_with_connect_info::<SocketAddr>()`, and graceful shutdown.
- Produces: exact dependency graph and a production build that creates and bundles `apps/remote-controller/dist` before packaging the desktop.

- [ ] **Step 1: Add exact Rust dependencies without staging the lockfile**

```toml
[dependencies]
axum = { version = "=0.8.4", features = ["ws"] }
base64 = "=0.22.1"
futures-util = "=0.3.34"
getrandom = "=0.3.4"
tokio = { version = "=1.53.1", features = ["macros", "net", "rt-multi-thread", "sync", "time"] }
tower-http = { version = "=0.6.11", features = ["fs", "set-header"] }

[dev-dependencies]
tokio-tungstenite = "=0.28.0"
```

Place these in `crates/gb-network/Cargo.toml`; retain exact existing `gb-core`, `serde`, and `serde_json` entries.

- [ ] **Step 2: Add the exact QR renderer**

```json
"qrcode.react": "4.2.0"
```

Add it to `apps/desktop/package.json` dependencies. `QRCodeSVG` receives only the parsed native `pairingUrl`; never render native SVG or HTML with `dangerouslySetInnerHTML`.

- [ ] **Step 3: Make controller assets a build prerequisite and Tauri resource**

In `tauri.conf.json`, replace the build commands with:

```json
"beforeDevCommand": "pnpm --filter @gameboy/remote-controller build && pnpm dev",
"beforeBuildCommand": "pnpm --filter @gameboy/remote-controller build && pnpm build"
```

Add this bundle resource mapping, relative to `apps/desktop/src-tauri/tauri.conf.json`:

```json
"resources": {
  "../../remote-controller/dist/": "controller"
}
```

- [ ] **Step 4: Resolve and commit manifests, then locks alone**

```bash
rtk pnpm install --lockfile-only
rtk cargo check -p gb-network
rtk git add crates/gb-network/Cargo.toml apps/desktop/package.json apps/desktop/src-tauri/tauri.conf.json
rtk git commit -m "build(remote): add local controller session dependencies"
rtk git add Cargo.lock
rtk git commit -m "build(rust): reconcile remote session lockfile"
rtk git add pnpm-lock.yaml
rtk git commit -m "build(web): lock desktop QR renderer"
```

Expected: each lock commit contains only its named lockfile; no PPU/APU work or generated `dist` file is staged.

### Task 2: Build the pure protocol-v1 session and rate limiter

**Files:**
- Modify: `crates/gb-network/src/session.rs`.
- Create: `crates/gb-network/src/rate_limit.rs`.
- Modify: `crates/gb-network/src/lib.rs`.
- Modify: `crates/gb-network/tests/session_contract.rs`.

**Interfaces:**
- Consumes: existing `ClientMessage`, `ServerMessage`, `SessionToken`, `ControllerConnectionId`, `InputSourceId`, and protocol-v1 fixtures.
- Produces: `ControllerEvent::Connected`, existing `Message`/`Disconnected`, `ControllerEventSink`, `SessionMachine`, and deterministic `InputRateLimiter`.

- [ ] **Step 1: Write failing connection and input-state tests**

Add tests that construct `SessionMachine::new(token, connection_id, InputSourceId::new(2), expires_at)` and prove:

```rust
assert!(matches!(
    machine.accept_hello(valid_hello, now),
    Ok(SessionAction::Connected {
        reply: ServerMessage::Welcome { version: ProtocolVersion::V1, .. },
        event: ControllerEvent::Connected { input_source, .. },
    }) if input_source == InputSourceId::new(2)
));
assert!(matches!(machine.apply(sequenced("state-sync", 41), now), Ok(SessionAction::Input(_))));
assert!(matches!(machine.apply(sequenced("button-down", 42), now), Ok(SessionAction::Input(_))));
assert!(matches!(machine.apply(sequenced("button-up", 43), now), Ok(SessionAction::Input(_))));
assert!(matches!(machine.apply(sequenced("ping", 44), now), Ok(SessionAction::Reply(ServerMessage::Pong { .. }))));
assert_eq!(machine.disconnect(), Some(ControllerEvent::Disconnected { connection_id, input_source: InputSourceId::new(2) }));
assert_eq!(machine.disconnect(), None);
```

Also test invalid/expired tokens, non-hello first frames, a second hello, duplicate/out-of-order sequences, `MAX_SAFE_SEQUENCE -> 0` wrap, and exactly one disconnect cleanup after authentication.

- [ ] **Step 2: Run the tests and observe the missing state machine**

```bash
rtk cargo test -p gb-network --test session_contract
```

Expected: FAIL because the new event/action/state types do not exist.

- [ ] **Step 3: Implement exact pure contracts**

Use these public shapes:

```rust
pub enum ControllerEvent {
    Connected { connection_id: ControllerConnectionId, input_source: InputSourceId },
    Message { connection_id: ControllerConnectionId, input_source: InputSourceId, message: ClientMessage },
    Disconnected { connection_id: ControllerConnectionId, input_source: InputSourceId },
}

pub trait ControllerEventSink: Send + Sync {
    fn publish(
        &self,
        event: ControllerEvent,
        received_at: std::time::Instant,
    ) -> Result<(), ControllerEventSinkError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControllerEventSinkError;

pub enum SessionAction {
    Connected { reply: ServerMessage, event: ControllerEvent },
    Input(ControllerEvent),
    Reply(ServerMessage),
    None,
}
```

`SessionMachine` owns authentication, the last accepted sequence, expiry, and an `emitted_disconnect` flag. The first sequenced message after `welcome` may use any safe sequence. Later messages must equal `previous + 1`, with only `MAX_SAFE_SEQUENCE -> 0` as wrap. It never opens a socket and never imports Tauri.

Extend the existing `ControllerEvent::input_source()` match to include `Connected`; retain `Debug + Clone + PartialEq + Eq` on the event and retain redacted `Debug` on `SessionToken`.

- [ ] **Step 4: Write and implement deterministic token-bucket tests**

```rust
let mut limiter = InputRateLimiter::new(240, 64, start);
for offset in 0..64 {
    assert!(limiter.allow(start + Duration::from_micros(offset)));
}
assert!(!limiter.allow(start + Duration::from_millis(1)));
assert!(limiter.allow(start + Duration::from_millis(5)));
```

The implementation is a deterministic integer token bucket: one token is `1_000_000_000` scaled units, elapsed nanoseconds refill `elapsed_nanos * 240`, and capacity is `64 * 1_000_000_000`. Clamp backward `Instant` input to zero elapsed, cap at capacity, deduct one token per sequenced message, and never use floating point or a background timer.

- [ ] **Step 5: Run protocol and session gates and commit**

```bash
rtk cargo test -p gb-network --test session_contract
rtk cargo test -p gb-network --test protocol_fixtures
rtk git add crates/gb-network/src/session.rs crates/gb-network/src/rate_limit.rs crates/gb-network/src/lib.rs crates/gb-network/tests/session_contract.rs
rtk git commit -m "feat(network): validate authenticated controller sessions"
```

### Task 3: Implement the bounded local HTTP/WebSocket server

**Files:**
- Create: `crates/gb-network/src/server.rs`.
- Create: `crates/gb-network/tests/live_session.rs`.
- Modify: `crates/gb-network/src/lib.rs`.

**Interfaces:**
- Consumes: Task 2 session machine/sink, an injected static asset root, an injected bind address for tests, and OS randomness in production.
- Produces: `ControllerServer::start(SessionServerConfig, Arc<dyn ControllerEventSink>)`, `PairingInfo`, `ControllerServer::shutdown`, and typed `NetworkError`.

- [ ] **Step 1: Write a failing real-socket happy-path test**

Use `tokio_tungstenite::connect_async` against `127.0.0.1:0`. Create an exact temporary directory with `index.html`, start the server with deterministic 32-byte entropy and source `2`, extract the URL-safe token from returned `PairingInfo::pairing_url`, fetch `/`, then connect `/controller` and send:

```rust
send_json(&mut socket, serde_json::json!({"type":"hello","version":"v1","token": pairing_token})).await;
send_json(&mut socket, serde_json::json!({"type":"state-sync","buttons":["left","a"],"sequence":7})).await;
send_json(&mut socket, serde_json::json!({"type":"button-up","button":"a","sequence":8})).await;
send_json(&mut socket, serde_json::json!({"type":"ping","sequence":9})).await;
```

Assert `welcome`, two input events, `pong` sequence `9`, one `Disconnected` after close, and that the pairing URL token equals the `URL_SAFE_NO_PAD` encoding of the deterministic 32 bytes.

- [ ] **Step 2: Run the live test and observe the missing server**

```bash
rtk cargo test -p gb-network --test live_session -- --nocapture
```

Expected: FAIL because `ControllerServer` and `SessionServerConfig` do not exist.

- [ ] **Step 3: Implement listener startup and graceful shutdown**

Expose these exact constructors:

```rust
pub trait SessionEntropy: Send + Sync {
    fn fill(&self, destination: &mut [u8]) -> Result<(), NetworkError>;
}

pub struct OsSessionEntropy;

pub struct SessionServerConfig {
    pub bind_address: IpAddr,
    pub controller_assets: PathBuf,
    pub input_source: InputSourceId,
    pub token_ttl: Duration,
    pub heartbeat_timeout: Duration,
    pub entropy: Arc<dyn SessionEntropy>,
}

pub struct PairingInfo {
    pub session_id: SessionId,
    pub pairing_url: String,
    pub expires_at_unix_ms: u64,
}

pub enum NetworkError {
    NoLanAddress,
    EntropyUnavailable,
    AssetsUnavailable,
    BindFailed,
    ThreadStartFailed,
    ServerUnavailable,
}

impl ControllerServer {
    pub fn start(config: SessionServerConfig, sink: Arc<dyn ControllerEventSink>) -> Result<(Self, PairingInfo), NetworkError>;
    pub fn shutdown(&self) -> Result<(), NetworkError>;
}
```

Add `SessionServerConfig::production(controller_assets, input_source) -> Result<Self, NetworkError>` to select `discover_lan_ipv4()`, `OsSessionEntropy`, the ten-minute token TTL, and 18-second heartbeat deadline. Tests construct the public fields directly with loopback and deterministic entropy.

Before spawning, `start` canonicalizes `controller_assets`, verifies that canonical `index.html` is a regular file, and otherwise returns `NetworkError::AssetsUnavailable`. It then creates a named OS thread, builds one Tokio multi-thread runtime, binds `TcpListener` to `(bind_address, 0)`, sends the bound address and pairing data back through a synchronous ready channel, and runs:

```rust
axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
    .with_graceful_shutdown(shutdown_signal)
    .await
```

`shutdown` is idempotent, invalidates the token, asks the accepted socket to send `controller-disconnected`, waits for the server thread, and guarantees one `Disconnected` event if authentication occurred.

- [ ] **Step 4: Implement route, frame, and connection limits**

Build a router with `get` on `/controller` and `ServeDir::new(controller_assets).append_index_html_on_directories(true)` as the static fallback. Add response headers `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`, and `Cache-Control: no-store` to the controller HTML response. Validate any browser `Origin` header against the exact advertised `http://host:port`; allow a missing origin only for non-browser/native tests. Before upgrade, reject non-controller paths from the WS handler. In the socket task:

- accept text only and reject any text over 4,096 bytes before JSON parsing;
- require `hello` within 5 seconds;
- claim one global active connection before `welcome`;
- send protocol-v1 `rejected` reasons, then close on terminal failures;
- run `tokio::time::timeout(18 seconds, socket.next())`, resetting it after every valid message;
- answer `ping` synchronously and publish only connected/input/disconnected events;
- treat `ControllerEventSinkError` as close code `1011`, never queue a second input event while the previous sink call is unresolved;
- release the one-controller claim and emit cleanup from one guard on every exit path.

- [ ] **Step 5: Add adversarial live tests**

Test invalid and expired token rejection, second-controller rejection while the first remains usable, a 4,097-byte frame, binary input, malformed JSON, wrong sequence, 65-message burst rejection, heartbeat timeout, explicit shutdown, reconnect after unexpected close, missing asset root, and `GET /not-present` returning 404 without directory traversal.

- [ ] **Step 6: Add production LAN address and token generation**

`discover_lan_ipv4()` uses a standard `UdpSocket` bound to `0.0.0.0:0`, connects to documentation-only address `192.0.2.1:80` without sending a datagram, reads `local_addr`, and rejects unspecified/loopback results as `NetworkError::NoLanAddress`. `generate_token()` fills `[u8; 32]` with `getrandom::fill` and encodes `URL_SAFE_NO_PAD`; IDs use independent 16-byte draws. Tests inject address/token factories and never depend on the host network.

- [ ] **Step 7: Verify and commit the server**

```bash
rtk cargo test -p gb-network --all-features
rtk cargo clippy -p gb-network --all-targets --all-features -- -D warnings
rtk git add crates/gb-network/src/server.rs crates/gb-network/tests/live_session.rs crates/gb-network/src/lib.rs
rtk git commit -m "feat(network): serve one authenticated LAN controller"
```

### Task 4: Replace the production mock with `SystemClock + GameBoy`

**Files:**
- Create: `apps/desktop/src-tauri/src/emulator/factory.rs`.
- Modify: `apps/desktop/src-tauri/src/emulator/mod.rs`.
- Test: inline tests in `factory.rs`.

**Interfaces:**
- Consumes: `GameBoy::new(clock, sample_rate)`, `Clock`, the negotiated `NonZeroU32`, and unchanged `CoreFactory::create()`.
- Produces: `SystemClock` and `GameBoyCoreFactory::new(sample_rate)`.

- [ ] **Step 1: Write failing real-factory tests**

```rust
let rate = NonZeroU32::new(48_000).expect("non-zero rate");
let factory = GameBoyCoreFactory::new(rate);
let mut core = factory.create();
let metadata = core.load_rom(&synthetic_valid_rom(&[0x00, 0x76]), None).expect("real ROM loads");
assert_eq!(metadata.mapper, MapperKind::RomOnly);
assert!(core.run_cycles(16).expect("real core advances").cycles_executed() > 0);
assert_eq!(core.drain_audio().sample_rate(), rate);
```

Build the synthetic ROM in the test by allocating exactly 32 KiB, writing a title, cartridge type `0x00`, ROM-size `0x00`, RAM-size `0x00`, program bytes at `0x0100`, and recomputing the header checksum over `0x0134..=0x014c`. No ROM file is read.

- [ ] **Step 2: Run the test and observe the missing factory**

```bash
rtk cargo test -p gameboy-desktop emulator::factory
```

Expected: FAIL because the production factory does not exist.

- [ ] **Step 3: Implement the clock and captured-rate factory**

```rust
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn unix_seconds(&self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
    }
}

pub(crate) struct GameBoyCoreFactory { sample_rate: NonZeroU32 }

impl CoreFactory for GameBoyCoreFactory {
    fn create(&self) -> Box<dyn RuntimeCore> {
        Box::new(GameBoy::new(SystemClock, self.sample_rate))
    }
}
```

Do not accept a clock, ROM, persistence path, or sample rate in `create`.

- [ ] **Step 4: Verify and commit**

```bash
rtk cargo test -p gameboy-desktop emulator::factory
rtk git add apps/desktop/src-tauri/src/emulator/factory.rs apps/desktop/src-tauri/src/emulator/mod.rs
rtk git commit -m "feat(desktop): construct the real Game Boy core"
```

### Task 5: Serialize source-aware remote input on the emulator worker

**Files:**
- Modify: `apps/desktop/src-tauri/src/emulator/runtime.rs`.
- Modify tests: inline `runtime.rs` tests.

**Interfaces:**
- Consumes: `ControllerEvent`, keyboard source `1`, remote source `2`, and the post-PED-49 runtime/audio loop.
- Produces: cloneable `DesktopRuntimeHandle`, `apply_controller_event`, complete remote snapshots, and latency completion at the same serialized boundary.

- [ ] **Step 1: Write failing coexistence and cleanup tests**

Extend `RecordingCore` tests to send keyboard A, remote Left+A, remote A-up, disconnect, and assert:

```rust
assert!(keyboard_state.is_pressed(Button::A));
assert!(remote_state.is_pressed(Button::Left));
assert!(!remote_state.is_pressed(Button::A));
assert_eq!(recording.clears.last(), Some(&InputSourceId::new(2)));
assert!(!recording.clears.contains(&InputSourceId::new(1)));
assert_eq!(runtime.snapshot().expect("ROM continues").phase, RuntimePhase::Running);
```

Also prove remote events received while no ROM is loaded update the retained snapshot and are applied after the next successful `OpenRom`; a failed desktop replacement preserves the remote snapshot for a later successful open without changing PED-37's existing `Error` lifecycle; restart preserves remote input; session disconnect clears only source `2`.

- [ ] **Step 2: Run focused tests and observe failure**

```bash
rtk cargo test -p gameboy-desktop emulator::runtime::tests::remote --all-features
```

Expected: FAIL because the runtime has only keyboard commands.

- [ ] **Step 3: Add a cloneable sender handle without changing the factory**

```rust
#[derive(Clone)]
pub(crate) struct DesktopRuntimeHandle { sender: SyncSender<RuntimeCommand> }

impl DesktopRuntime {
    pub(crate) fn handle(&self) -> DesktopRuntimeHandle;
}

impl DesktopRuntimeHandle {
    pub(crate) fn apply_controller_event(&self, event: ControllerEvent) -> RuntimeResult<()>;
}
```

Reuse the existing bounded request/reply path and two-second timeout. Do not clone the worker `JoinHandle`, audio output, core, or observer.

- [ ] **Step 4: Apply complete per-source snapshots in the worker**

Add `RuntimeCommand::ControllerEvent` and a worker-owned `HashMap<InputSourceId, JoypadState>`. Map network `Button` to core `Button` exhaustively. `state-sync` replaces the complete source snapshot; button down/up mutate it; `Disconnected` removes it and calls `clear_input_source`. Ignore authenticated `Connected` for core input. `Hello` and `Ping` must never reach this method; return `InvalidLifecycle` if they do.

The handle's remote-event request retries `TrySendError::Full(returned_command)` every 1 ms for at most 100 ms, then waits on the ordinary bounded reply timeout. This path is safe because the WebSocket task waits for each sink result and therefore permits only one remote command in flight. Add a saturation regression: temporarily block the worker, fill the queue, release it within 100 ms, deliver `Disconnected`, and assert source `2` is cleared. Do not change the existing fail-fast behavior of UI lifecycle requests.

On successful `OpenRom`, apply every retained remote snapshot to the new core after `load_rom`. Preserve the merged PED-49 order for audio teardown/drain and do not alter frame/audio deadlines.

- [ ] **Step 5: Run runtime, video, and audio regression gates and commit**

```bash
rtk cargo test -p gameboy-desktop emulator::runtime --all-features
rtk cargo test -p gameboy-desktop video --all-features
rtk cargo test -p gameboy-desktop audio --all-features
rtk git add apps/desktop/src-tauri/src/emulator/runtime.rs
rtk git commit -m "feat(desktop): serialize remote input with keyboard input"
```

### Task 6: Own session lifecycle and latency in the desktop adapter

**Files:**
- Create: `apps/desktop/src-tauri/src/remote/mod.rs`.
- Create: `apps/desktop/src-tauri/src/remote/contracts.rs`.
- Create: `apps/desktop/src-tauri/src/remote/manager.rs`.

**Interfaces:**
- Consumes: `ControllerServer`, `DesktopRuntimeHandle`, controller resource path, and remote source `2`.
- Produces: `RemoteSessionManager::{start,end,snapshot,subscribe,shutdown}`, typed snapshots, and bounded latency statistics.

- [ ] **Step 1: Write failing manager lifecycle tests**

Use a fake server factory plus a real `DesktopRuntime` recording core. Assert exact transitions:

```text
off -> waiting(pairingUrl, expiresAtUnixMs) -> connected(controllerId)
connected -> waiting after unexpected disconnect
waiting/connected -> off after end
off -> error(no-lan-address | bind-failed | assets-unavailable) on start failure
```

Assert `end` and `shutdown` are idempotent, the token disappears from every `off/error` snapshot, ROM phase is unchanged, and keyboard source is never cleared.

- [ ] **Step 2: Define the serializable contract**

```rust
pub enum RemotePhase { Off, Waiting, Connected, Error }
pub enum RemoteErrorCode {
    NoLanAddress,
    BindFailed,
    AssetsUnavailable,
    ServerFailed,
    RuntimeUnavailable,
    InvalidLifecycle,
}
pub struct RemoteLatency { pub samples: u64, pub last_ms: u64, pub p95_ms: u64 }
pub struct RemoteSnapshot {
    pub phase: RemotePhase,
    pub pairing_url: Option<String>,
    pub expires_at_unix_ms: Option<u64>,
    pub controller_id: Option<String>,
    pub latency: Option<RemoteLatency>,
    pub error: Option<RemoteError>,
}
pub enum RemoteEvent { Snapshot { snapshot: RemoteSnapshot } }
```

Use kebab-case enum values and camelCase struct fields. Never serialize the raw `SessionToken` separately; it appears only inside `pairingUrl` while waiting/connected.

- [ ] **Step 3: Implement manager ownership and observation**

`RemoteSessionManager` owns `Mutex<RemoteModel>`, optional `ControllerServer`, optional observer, a server factory, and the `DesktopRuntimeHandle`. Its sink returns `Result<(), ControllerEventSinkError>`, forwards each input event synchronously through `apply_controller_event`, and after success records `received_at.elapsed()` in a fixed 128-sample `VecDeque<Duration>`. Calculate p95 by sorting a copied maximum-128-value vector only when `snapshot()` is requested, not per input.

Never hold the model mutex while starting/stopping/joining the server, waiting for an emulator reply, or publishing a Tauri channel event. Use an internal `start_in_progress` flag: `start()` returns the existing waiting/connected snapshot idempotently when a server is active and returns typed `InvalidLifecycle` only for a concurrent start already in progress. Move the server handle out of the mutex before shutdown, then reacquire the mutex to publish the final snapshot; this prevents the server's disconnect callback from deadlocking against `end()`.

Connected/disconnected events update/publish remote status. Runtime forwarding errors end the controller connection and publish `runtime-unavailable`, but do not call emulator `pause`, `close`, or `shutdown`.

- [ ] **Step 4: Verify and commit**

```bash
rtk cargo test -p gameboy-desktop remote::manager --all-features
rtk git add apps/desktop/src-tauri/src/remote
rtk git commit -m "feat(desktop): manage remote controller sessions"
```

### Task 7: Register commands, packaged assets, real factory, and shutdown order

**Files:**
- Create: `apps/desktop/src-tauri/src/remote/commands.rs`.
- Modify: `apps/desktop/src-tauri/src/lib.rs`.
- Modify: `apps/desktop/src-tauri/build.rs`.
- Modify: `apps/desktop/src-tauri/permissions/emulator.toml`.
- Modify: `apps/desktop/src-tauri/capabilities/default.json` only if a new named permission is used.

**Interfaces:**
- Consumes: Tasks 4-6 and the exact post-PED-49 `DesktopRuntime::spawn(factory, audio_factory, runtime_sample_rate, prepared_output)` call.
- Produces: `remote_snapshot`, `subscribe_remote`, `start_remote_session`, `end_remote_session` Tauri commands and production runtime registration.

- [ ] **Step 1: Write failing command-adapter tests**

Test same-named `pub(crate)` helpers directly with a manager backed by fake server/runtime factories. Verify snapshot, subscription's immediate event, start, repeated active start returning the same snapshot without a second listener, concurrent-start `InvalidLifecycle`, end, and typed failures. Do not construct `tauri::State` in tests.

- [ ] **Step 2: Implement thin command adapters**

```rust
#[tauri::command]
pub fn remote_snapshot(state: State<'_, RemoteSessionManager>) -> RemoteResult<RemoteSnapshot>;
#[tauri::command]
pub fn subscribe_remote(events: Channel<RemoteEvent>, state: State<'_, RemoteSessionManager>) -> RemoteResult<RemoteSnapshot>;
#[tauri::command]
pub fn start_remote_session(state: State<'_, RemoteSessionManager>) -> RemoteResult<RemoteSnapshot>;
#[tauri::command]
pub fn end_remote_session(state: State<'_, RemoteSessionManager>) -> RemoteResult<RemoteSnapshot>;
```

The adapter performs no token generation, address discovery, state transition, or core call.

- [ ] **Step 3: Resolve controller assets and register real runtime in `setup`**

Inside Tauri `setup`, resolve `app.path().resource_dir()?.join("controller")`; reject a missing `index.html` as `AssetsUnavailable`. Open CPAL once using the merged PED-49 factory, derive `runtime_sample_rate` or explicit 48,000 Hz fallback exactly as PED-49 implemented, then construct:

```rust
let core_factory = Arc::new(GameBoyCoreFactory::new(runtime_sample_rate));
let runtime = DesktopRuntime::spawn(core_factory, audio_factory, runtime_sample_rate, prepared_output);
let remote = RemoteSessionManager::new(runtime.handle(), controller_assets);
app.manage(runtime);
app.manage(remote);
```

Delete only the production import/registration of `ContractMockCoreFactory`; retain its module and tests.

- [ ] **Step 4: Register metadata and permissions**

Add all four command names to `tauri::generate_handler!`, `build.rs`'s `AppManifest`, and the existing `allow-emulator-runtime` command list. Regenerate/check Tauri metadata through the normal build; do not hand-edit `gen/schemas/**`.

- [ ] **Step 5: Enforce shutdown order**

On `ExitRequested`, call `RemoteSessionManager::shutdown()` first and `DesktopRuntime::shutdown()` second. This allows source `2` cleanup to traverse the still-live emulator command queue. Both calls remain idempotent for managed-state `Drop`.

- [ ] **Step 6: Verify production registration and commit**

```bash
rtk cargo test -p gameboy-desktop remote::commands emulator::factory --all-features
rtk cargo check -p gameboy-desktop --all-features
rtk rg -n "ContractMockCoreFactory" apps/desktop/src-tauri/src/lib.rs
rtk git add apps/desktop/src-tauri/src/remote/commands.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/build.rs apps/desktop/src-tauri/permissions/emulator.toml apps/desktop/src-tauri/capabilities/default.json
rtk git commit -m "feat(desktop): register real core and remote commands"
```

Expected: the final `rg` exits 1; the mock is absent from production registration.

### Task 8: Add typed desktop remote-session client and hook

**Files:**
- Create: `apps/desktop/src/features/remote-controller/remote-types.ts`.
- Create: `apps/desktop/src/features/remote-controller/remote-types.test.ts`.
- Create: `apps/desktop/src/features/remote-controller/remote-client.ts`.
- Create: `apps/desktop/src/features/remote-controller/remote-client.test.ts`.
- Create: `apps/desktop/src/features/remote-controller/use-remote-session.ts`.
- Create: `apps/desktop/src/features/remote-controller/use-remote-session.test.tsx`.

**Interfaces:**
- Consumes: Task 6 JSON shape and four Task 7 commands.
- Produces: `RemoteSessionClient`, `RemoteSessionView`, Zod parsing, subscription, and normalized typed errors.

- [ ] **Step 1: Write failing strict-parser tests**

Test valid `off`, `waiting`, `connected`, and `error` snapshots. Reject a waiting snapshot without URL/expiry, connected without controller ID, off with leaked URL/token, negative latency, unknown keys, and unknown phase/error code. The exact native error-code enum is `no-lan-address | bind-failed | assets-unavailable | server-failed | runtime-unavailable | invalid-lifecycle`.

- [ ] **Step 2: Implement discriminated Zod schemas**

Use this exact TypeScript union:

```ts
export type RemoteSnapshot =
  | { phase: 'off'; pairingUrl: null; expiresAtUnixMs: null; controllerId: null; latency: null; error: null }
  | { phase: 'waiting'; pairingUrl: string; expiresAtUnixMs: number; controllerId: null; latency: RemoteLatency | null; error: null }
  | { phase: 'connected'; pairingUrl: string; expiresAtUnixMs: number; controllerId: string; latency: RemoteLatency | null; error: null }
  | { phase: 'error'; pairingUrl: null; expiresAtUnixMs: null; controllerId: null; latency: RemoteLatency | null; error: RemoteError }
```

Require `http:` or `https:` URL, safe integer epoch/latency fields, and strict objects.

- [ ] **Step 3: Write failing client/channel tests**

Mock `invoke` and `Channel`. Assert exact commands/arguments, immediate snapshot parsing, pushed event parsing, and normalization of native typed errors versus unknown failures.

- [ ] **Step 4: Implement the adapter and hook**

```ts
export interface RemoteSessionClient {
  subscribe(onSnapshot: (snapshot: RemoteSnapshot) => void): Promise<RemoteSnapshot>
  snapshot(): Promise<RemoteSnapshot>
  start(): Promise<RemoteSnapshot>
  end(): Promise<RemoteSnapshot>
}
```

`useRemoteSession` subscribes once, ignores events after unmount, applies action results, exposes `busy: 'starting' | 'ending' | null`, and never writes emulator lifecycle state.

- [ ] **Step 5: Verify and commit**

```bash
rtk pnpm --filter @gameboy/desktop test -- src/features/remote-controller
rtk pnpm --filter @gameboy/desktop typecheck
rtk git add apps/desktop/src/features/remote-controller/remote-types.ts apps/desktop/src/features/remote-controller/remote-types.test.ts apps/desktop/src/features/remote-controller/remote-client.ts apps/desktop/src/features/remote-controller/remote-client.test.ts apps/desktop/src/features/remote-controller/use-remote-session.ts apps/desktop/src/features/remote-controller/use-remote-session.test.tsx
rtk git commit -m "feat(desktop): add typed remote session client"
```

### Task 9: Render session actions, local URL, QR Code, and errors

**Files:**
- Create: `apps/desktop/src/features/remote-controller/RemoteControllerPanel.tsx`.
- Create: `apps/desktop/src/features/remote-controller/RemoteControllerPanel.test.tsx`.
- Modify: `apps/desktop/src/pages/EmulatorPage.tsx`.
- Modify: `apps/desktop/src/pages/EmulatorPage.test.tsx`.
- Modify: `apps/desktop/src/styles.css`.

**Interfaces:**
- Consumes: `useRemoteSession`, `QRCodeSVG`, existing Card/Button/Badge/Alert components, and protocol literal `v1`.
- Produces: accessible start/end controls, QR and copyable URL, waiting/connected/expired/error state, and optional latency display.

- [ ] **Step 1: Write failing panel tests**

With an injected `RemoteSessionClient`, assert:

- off shows `Start mobile controller` and no QR/token;
- waiting shows `Scan to connect`, a QR with the exact URL, a selectable URL, expiry text, and `End session`;
- connected shows `Mobile controller connected`, controller ID, and `End session`;
- ending disables actions until resolution;
- all six typed failures show actionable copy while ROM controls remain enabled;
- latency appears as `Local input p95: N ms` only after at least one sample.

Mock only `qrcode.react` rendering in unit tests; assert its `value` prop equals `pairingUrl`.

- [ ] **Step 2: Implement the focused panel**

Render `<QRCodeSVG value={snapshot.pairingUrl} size={176} level="M" marginSize={2} title="Mobile controller pairing QR Code" />`. Do not log the URL, token, or QR value. Error headings map all six native error codes and recommend keyboard play where recovery is not immediate.

- [ ] **Step 3: Replace the inert footer without coupling hooks**

Add optional `remoteSessionClient?: RemoteSessionClient` to `EmulatorPageProps` and pass it to `RemoteControllerPanel`. Remove the PED-39 tooltip and inert `aria-disabled` button. Do not pass `runtime`, ROM paths, or keyboard mappings into the panel.

- [ ] **Step 4: Style responsive QR presentation and verify**

Add a bounded panel that wraps URL text, preserves QR contrast, and collapses actions below 640px. Keep existing viewport aspect ratio and keyboard dialog styles unchanged.

```bash
rtk pnpm --filter @gameboy/desktop test -- src/features/remote-controller src/pages/EmulatorPage.test.tsx
rtk pnpm --filter @gameboy/desktop lint
rtk pnpm --filter @gameboy/desktop typecheck
rtk pnpm --filter @gameboy/desktop build
rtk git add apps/desktop/src/features/remote-controller/RemoteControllerPanel.tsx apps/desktop/src/features/remote-controller/RemoteControllerPanel.test.tsx apps/desktop/src/pages/EmulatorPage.tsx apps/desktop/src/pages/EmulatorPage.test.tsx apps/desktop/src/styles.css
rtk git commit -m "feat(desktop): present QR controller sessions"
```

### Task 10: Prove real protocol-to-core behavior and reconnect

**Files:**
- Create: `apps/desktop/src-tauri/tests/remote_runtime.rs`.
- Modify implementation files only if a failing test exposes a defect; add the regression beside it.

**Interfaces:**
- Consumes: real `ControllerServer`, real desktop manager/runtime, recording core factory, real protocol JSON, and static fixture assets.
- Produces: automated acceptance evidence from WebSocket frame to serialized core input.

- [ ] **Step 1: Write the end-to-end integration fixture**

Start a recording runtime in `Running`, press keyboard A, start a loopback remote session, connect a real Tungstenite client, authenticate, and send remote Left+A. Assert the recording core has source `1` A and source `2` Left+A. Drop the socket without `button-up`; assert source `2` is cleared, source `1` remains A, and runtime stays `Running`.

- [ ] **Step 2: Cover one-controller, reconnect, lifecycle, and latency**

In the same test binary prove:

1. invalid token is rejected;
2. a second controller is rejected while the first continues sending input;
3. disconnect then reconnect with the same unexpired QR token succeeds;
4. first post-reconnect `state-sync` restores the complete desired state;
5. pause/start/restart/open replacement do not end the remote session;
6. explicit session end clears source `2`, retains keyboard source `1`, and invalidates the old token;
7. 200 loopback input transitions produce 200 latency samples, bounded storage of 128, and p95 below 100 ms on an otherwise idle test host.

- [ ] **Step 3: Verify repeatability and commit**

```bash
rtk cargo test -p gameboy-desktop --test remote_runtime --all-features -- --nocapture
rtk cargo test -p gameboy-desktop --test remote_runtime --all-features -- --nocapture
rtk git add apps/desktop/src-tauri/tests/remote_runtime.rs
rtk git commit -m "test(desktop): verify remote input end to end"
```

### Task 11: Validate the existing mobile client against the real server

**Files:**
- Modify: `apps/remote-controller/src/features/session/**` only if a real-server test exposes a protocol-v1 integration defect.
- Test: add a regression test beside any correction.

**Interfaces:**
- Consumes: unchanged PED-38 pairing URL/parser, client heartbeat, reconnect state sync, and Task 3 real server.
- Produces: evidence that no protocol v1 or mobile-source import change is required.

- [ ] **Step 1: Run the complete mobile gate unchanged**

```bash
rtk pnpm --filter @gameboy/remote-controller lint
rtk pnpm --filter @gameboy/remote-controller typecheck
rtk pnpm --filter @gameboy/remote-controller test
rtk pnpm --filter @gameboy/remote-controller build
```

- [ ] **Step 2: Perform one real browser handshake**

Start the desktop session, open the displayed HTTP URL in a browser on the same machine, and verify the browser requests `/controller`, sends `hello`, receives `welcome`, immediately sends `state-sync`, receives matching `pong`, and reconnects after a forced socket close. Use browser network inspection without printing the token into test logs or issue comments.

- [ ] **Step 3: Correct only proven integration defects**

If the real server reveals a client defect, first add a failing test using `MockControllerServer`, then make the smallest fix under `apps/remote-controller/src/features/session/**`. Do not add a message, field, rejection reason, or protocol version.

```bash
rtk git add apps/remote-controller/src/features/session
rtk git commit -m "fix(remote): align client with live protocol session"
```

Expected: no commit when the existing PED-38 client already satisfies the real server.

### Task 12: Run full, macOS Apple Silicon, Windows x64, and manual LAN gates

**Files:**
- Create: `docs/testing/ped-39-remote-integration.md`.
- Modify: `docs/architecture/runtime-boundaries.md`.
- Modify: `docs/architecture/protocol-v1.md`.
- Modify implementation only for defects reproduced by a failing test.

**Interfaces:**
- Consumes: merged PED-39 and a legally redistributable supported ROM already provisioned by the developer.
- Produces: review-ready platform evidence and final Linear synchronization.

- [ ] **Step 1: Update architecture documents from future to actual behavior**

In `runtime-boundaries.md`, record source `1`/`2`, real `SystemClock + GameBoy` production registration, the locally served controller assets, exact bind/route/token/one-controller/cleanup boundaries, and remote-before-runtime shutdown. In `protocol-v1.md`, retain every existing message/field and document only transport rules: hello-first, unified contiguous sequence with safe wrap, 4,096-byte text limit, token-bucket rate, 18-second server deadline, origin check, and internal Connected/Message/Disconnected events. State explicitly that neither document changes protocol v1.

- [ ] **Step 2: Run the complete workspace gate from a clean diff**

```bash
rtk pnpm lint
rtk pnpm typecheck
rtk pnpm test
rtk pnpm build
rtk cargo fmt --all --check
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk cargo test --workspace --all-features
rtk cargo test -p gb-core --no-default-features
rtk cargo tree -p gb-core
rtk rg "apps/desktop|src-tauri|gb-network" apps/remote-controller/src apps/remote-controller/package.json
rtk git diff --check
```

Expected: every command except the final boundary `rg` exits 0; that `rg` exits 1 because the mobile app imports no desktop/network source. Protocol fixtures are unchanged; the `gb-core` tree has no Axum/Tokio/Tauri/platform dependency.

- [ ] **Step 3: Run the macOS Apple Silicon gate**

On Apple Silicon macOS:

```bash
rtk cargo test -p gb-network --all-features
rtk cargo test -p gameboy-desktop --all-features
rtk pnpm --filter @gameboy/desktop tauri build --target aarch64-apple-darwin
```

Launch the packaged app, start/end/restart a session twice, verify the advertised address is the Mac's LAN address and not `0.0.0.0`/loopback, and verify quitting invalidates the URL.

- [ ] **Step 4: Run the Windows x64 gate**

On a native Windows x64 runner with MSVC Build Tools:

```bash
rtk cargo test -p gb-network --all-features
rtk cargo test -p gameboy-desktop --all-features
rtk pnpm --filter @gameboy/desktop tauri build --target x86_64-pc-windows-msvc
```

Launch the packaged `.exe`, allow the app only on the private network when Windows Firewall prompts, verify start/end/reconnect and one-controller rejection, and confirm no inbound listener remains after ending the session or exiting. A compile-only cross-check is useful but does not replace this native gate.

- [ ] **Step 5: Perform real ROM + phone acceptance**

With one redistributable ROM already provisioned:

1. open and start the ROM with keyboard only;
2. start a remote session and scan the QR from one iOS Safari or Android Chrome device on the same LAN;
3. press D-pad + A simultaneously and observe gameplay;
4. hold keyboard A while pressing/releasing remote Left, proving keyboard A remains active;
5. disable phone Wi-Fi mid-press and prove remote buttons clear while ROM/audio/video/keyboard continue;
6. re-enable Wi-Fi and prove reconnect plus `state-sync` restores current touch state without restarting the ROM;
7. try a second phone and observe `Another controller is connected` while the first remains active;
8. end the session and prove keyboard play continues;
9. leave one session running for 15 minutes, verifying heartbeat stability, latency p95 under 50 ms on the LAN, bounded memory, and token expiry behavior.

- [ ] **Step 6: Record evidence without secrets**

Create `docs/testing/ped-39-remote-integration.md` with date, commit, OS/architecture, browser/device, ROM name/license/source (not ROM bytes), automated command results, session scenarios, p95 latency, and known platform/firewall notes. Redact the query token and do not paste pairing URLs.

- [ ] **Step 7: Commit evidence and synchronize Linear**

```bash
rtk git add docs/testing/ped-39-remote-integration.md docs/architecture/runtime-boundaries.md docs/architecture/protocol-v1.md
rtk git commit -m "docs: record PED-39 remote integration evidence"
rtk git status
```

Move PED-39 to `In Review` only after automated and both native platform gates pass. Complete an independent Standards + Spec review. Move it to `Done` only when review findings are fixed and all issue acceptance criteria pass; leave PED-40 unchanged.

## Final Acceptance Checklist

- Production Tauri registration constructs `GameBoy<SystemClock>` at the audio-negotiated sample rate through unchanged `CoreFactory::create()`.
- One mobile browser loads bundled local assets from the QR URL and authenticates over `/controller` with protocol `v1`.
- A second authenticated attempt is rejected without disconnecting the active controller.
- Invalid/expired tokens, oversized/malformed frames, rate abuse, and unexpected routes are rejected without exposing ROM/filesystem data.
- Button down/up and complete state-sync reach source `2`; keyboard remains source `1`; their union occurs only inside `gb-core`.
- WebSocket close, heartbeat timeout, explicit end, and app exit clear source `2` exactly once and do not restart/close/pause the ROM.
- Reconnect works while the token remains valid and its first input is the mobile client's complete `state-sync`.
- Remote errors degrade to keyboard play; video and audio keep running.
- Local input latency is measured from decode to worker application, stored in a bounded 128-sample window, and meets the recorded LAN p95 target.
- macOS Apple Silicon and Windows x64 native builds/session gates pass.
- Protocol fixtures and `packages/protocol` remain unchanged; `gb-core` remains platform/network independent.

## Context7 Reference Used

Axum 0.8.4 documentation confirmed `WebSocketUpgrade::on_upgrade`, `Router::route`, `TcpListener` + `axum::serve`, `into_make_service_with_connect_info::<SocketAddr>()`, and `with_graceful_shutdown`. Executors should keep all socket details inside `gb-network`; no Axum type crosses into the desktop or core public contracts.
