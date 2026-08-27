# PED-34 Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the compiling monorepo, platform-independent core contracts, versioned remote protocol, React/Tauri desktop shell, mobile shell, documentation, and CI foundation required to unblock PED-35, PED-37, and PED-38.

**Architecture:** A pnpm workspace hosts the two React applications and the TypeScript protocol package; a Cargo workspace hosts `gb-core`, `gb-network`, and the Tauri runtime crate. Protocol fixtures and narrow Rust contracts freeze cross-lane boundaries before parallel implementation begins.

**Tech Stack:** Node.js 24.20.0 LTS, pnpm, TypeScript, React, Vite, Tauri 2, Rust stable, Cargo, Zod, TanStack Router, TanStack Query, Tailwind CSS, shadcn/ui, Vitest, Testing Library, Biome

**Spec:** `docs/superpowers/specs/2026-08-27-game-boy-emulator-design.md`

## Global Constraints

- Target macOS Apple Silicon and Windows x64.
- Use Node.js `24.20.0`, the latest LTS verified on 2026-08-27; re-check before execution and update only if a newer LTS patch exists.
- Use pnpm and save every JavaScript dependency at an exact version.
- Use Rust stable and commit `Cargo.lock` because the workspace produces applications.
- `gb-core` must compile with no Tauri, React, filesystem, network, or platform-audio dependency.
- Protocol wire version is the string `v1` and messages use JSON for controller traffic.
- Do not implement CPU instructions, PPU, APU, real ROM lifecycle, or a listening network server in PED-34.
- Keep generated application UI minimal; PED-37 and PED-38 own product behavior after the contracts are frozen.

---

### Task 1: Create the root workspace and quality baseline

**Files:**
- Create: `package.json`.
- Create: `pnpm-workspace.yaml`.
- Create: `.npmrc`.
- Create: `.tool-versions`.
- Create: `.editorconfig`.
- Create: `.gitignore`.
- Create: `biome.json`.
- Create: `Cargo.toml`.
- Create: `rust-toolchain.toml`.
- Create: `AGENTS.md`.
- Create: `README.md`.

**Interfaces:**
- Consumes: none.
- Produces: root `pnpm` and Cargo workspaces plus discoverable lint, typecheck, test, and build commands used by all later tasks.

- [ ] **Step 1: Verify execution-time tool versions**

Run:

```bash
node --version
corepack pnpm --version
rustc --version
cargo --version
```

Expected: Node reports `v24.20.0` or a newer v24 LTS patch, pnpm is available through Corepack, and Rust/Cargo use stable toolchains. If Node differs, install the `.tool-versions` value before scaffolding.

- [ ] **Step 2: Create the root manifests**

Create `package.json` with this initial shape:

```json
{
  "name": "gameboy-emulator",
  "private": true,
  "scripts": {
    "build": "pnpm -r --if-present build",
    "dev:desktop": "pnpm --filter @gameboy/desktop tauri dev",
    "dev:remote": "pnpm --filter @gameboy/remote-controller dev",
    "lint": "biome check .",
    "lint:fix": "biome check --write .",
    "typecheck": "pnpm -r --if-present typecheck",
    "test": "pnpm -r --if-present test",
    "test:watch": "pnpm -r --parallel --if-present test:watch",
    "test:coverage": "pnpm -r --if-present test:coverage"
  }
}
```

Then run:

```bash
corepack use pnpm@latest
pnpm add --save-exact -Dw @biomejs/biome@latest
```

Expected: Corepack writes an exact `packageManager` value and pnpm writes an exact Biome version without `^` or `~`.

Create `pnpm-workspace.yaml`:

```yaml
packages:
  - apps/*
  - packages/*
```

Create `.npmrc`:

```ini
save-exact=true
shared-workspace-lockfile=true
strict-peer-dependencies=true
```

Create `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
  "apps/desktop/src-tauri",
  "crates/gb-core",
  "crates/gb-network",
]

[workspace.package]
edition = "2024"
license = "MIT"
version = "0.1.0"

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = "deny"
pedantic = "warn"
```

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["clippy", "rustfmt"]
profile = "minimal"
```

Create `.tool-versions` with `nodejs 24.20.0`, `.editorconfig` from project standards, and a `.gitignore` covering `node_modules`, `target`, `dist`, coverage, platform build products, `.DS_Store`, and local ROM/save files (`*.gb`, `*.gbc`, `*.sav`).

- [ ] **Step 3: Configure Biome**

Create `biome.json` with the installed schema path, the required two-space/LF/120-column/single-quote/no-semicolon settings, enabled import organization, and import groups in this order: Node built-ins, React, third-party, `@/errors`, `@/lib`, `@/config`, `@/services`, `@/hooks`, `@/components`, `@/features`, other `@/**`, relative paths.

- [ ] **Step 4: Document contributor constraints**

Create `AGENTS.md` recording module ownership, dependency order, protocol/core contract change rules, exact-version policy, root verification commands, test-ROM licensing policy, and the ban on platform dependencies inside `gb-core`. Create `README.md` with prerequisites and root commands but no unsupported compatibility claims.

- [ ] **Step 5: Verify the empty workspaces resolve**

Run:

```bash
pnpm install
pnpm lint
cargo metadata --no-deps --format-version 1
```

Expected: pnpm creates `pnpm-lock.yaml`; Biome accepts the root files; Cargo will report missing members until Tasks 3-5 add them, so run `cargo metadata` again at Task 6 and require success there.

- [ ] **Step 6: Commit the root baseline**

```bash
git add package.json pnpm-workspace.yaml pnpm-lock.yaml .npmrc .tool-versions .editorconfig .gitignore biome.json Cargo.toml rust-toolchain.toml AGENTS.md README.md
git commit -m "chore: establish emulator workspace standards"
```

### Task 2: Define protocol v1 in TypeScript with shared fixtures

**Files:**
- Create: `packages/protocol/package.json`.
- Create: `packages/protocol/tsconfig.json`.
- Create: `packages/protocol/vitest.config.ts`.
- Create: `packages/protocol/src/messages.ts`.
- Create: `packages/protocol/src/index.ts`.
- Create: `packages/protocol/test/messages.test.ts`.
- Create: `packages/protocol/fixtures/protocol-v1.json`.

**Interfaces:**
- Consumes: Zod runtime validation.
- Produces: `PROTOCOL_VERSION`, `Button`, `ClientMessage`, `ServerMessage`, `parseClientMessage`, and `parseServerMessage` for desktop, mobile, and Rust mirror tests.

- [ ] **Step 1: Add exact protocol dependencies**

Run:

```bash
pnpm --filter @gameboy/protocol add --save-exact zod
pnpm --filter @gameboy/protocol add --save-exact -D typescript vitest @vitest/coverage-v8
```

If the package does not exist yet, create its manifest first with name `@gameboy/protocol`, `type: module`, exports for `./src/index.ts`, and scripts `build`, `lint`, `typecheck`, `test`, `test:watch`, and `test:coverage`.

- [ ] **Step 2: Write failing protocol tests**

Test these exact valid messages and rejection cases:

```ts
expect(parseClientMessage({ type: 'hello', version: 'v1', token: 'abc' })).toEqual({
  type: 'hello',
  version: 'v1',
  token: 'abc'
})
expect(parseClientMessage({ type: 'button-down', button: 'a', sequence: 1 })).toMatchObject({ button: 'a' })
expect(parseClientMessage({ type: 'button-up', button: 'start', sequence: 2 })).toMatchObject({ button: 'start' })
expect(() => parseClientMessage({ type: 'button-down', button: 'turbo', sequence: 3 })).toThrow()
expect(() => parseClientMessage({ type: 'hello', version: 'v2', token: 'abc' })).toThrow()
```

Also cover `state-sync`, `ping`, `welcome`, `rejected`, `pong`, and `controller-disconnected` fixtures.

- [ ] **Step 3: Run the test and confirm failure**

Run `pnpm --filter @gameboy/protocol test`.

Expected: FAIL because `messages.ts` and exported parsers do not exist.

- [ ] **Step 4: Implement the protocol schema**

Use Zod discriminated unions. Export buttons exactly as `up`, `down`, `left`, `right`, `a`, `b`, `start`, and `select`. Every post-handshake client input includes an integer `sequence >= 0`; `state-sync` includes the full unique button array. `hello` is the only client message containing the token. Rejection reasons are `invalid-token`, `unsupported-version`, `controller-already-connected`, and `malformed-message`.

- [ ] **Step 5: Add the canonical fixture file**

`protocol-v1.json` contains one valid JSON object for every message variant and arrays named `validClientMessages`, `validServerMessages`, and `invalidMessages`. Tests load the fixture and assert that the appropriate parser accepts or rejects every item.

- [ ] **Step 6: Run protocol verification and commit**

```bash
pnpm --filter @gameboy/protocol typecheck
pnpm --filter @gameboy/protocol test
pnpm --filter @gameboy/protocol build
git add packages/protocol pnpm-lock.yaml
git commit -m "feat(protocol): define versioned controller messages"
```

Expected: all commands pass and the fixture has no untested entry.

### Task 3: Define platform-independent gb-core contracts

**Files:**
- Create: `crates/gb-core/Cargo.toml`.
- Create: `crates/gb-core/src/lib.rs`.
- Create: `crates/gb-core/src/contracts/mod.rs`.
- Create: `crates/gb-core/src/contracts/audio.rs`.
- Create: `crates/gb-core/src/contracts/cartridge.rs`.
- Create: `crates/gb-core/src/contracts/clock.rs`.
- Create: `crates/gb-core/src/contracts/emulator.rs`.
- Create: `crates/gb-core/src/contracts/frame.rs`.
- Create: `crates/gb-core/src/contracts/input.rs`.
- Create: `crates/gb-core/tests/contracts.rs`.

**Interfaces:**
- Consumes: Rust standard library only.
- Produces: `AudioBatch`, `BatteryState`, `Button`, `CartridgeMetadata`, `Clock`, `CoreError`, `EmulatorCore`, `Frame`, `InputSourceId`, `JoypadState`, `RunOutcome`, `SCREEN_WIDTH`, and `SCREEN_HEIGHT`.

- [ ] **Step 1: Write failing contract tests**

Test exact invariants:

```rust
assert_eq!(SCREEN_WIDTH, 160);
assert_eq!(SCREEN_HEIGHT, 144);
assert_eq!(Frame::blank().rgba().len(), 160 * 144 * 4);

let mut input = JoypadState::default();
input.press(Button::A);
input.press(Button::Left);
assert!(input.is_pressed(Button::A));
assert!(input.is_pressed(Button::Left));
input.release(Button::A);
assert!(!input.is_pressed(Button::A));
```

Add a fake `Clock` returning a fixed Unix-second value and a compile-time fake `EmulatorCore` implementing every required method.

- [ ] **Step 2: Run and confirm failure**

Run `cargo test -p gb-core --test contracts`.

Expected: FAIL because the crate and public contract modules do not exist.

- [ ] **Step 3: Implement the minimal contracts**

Define `EmulatorCore` with these signatures:

```rust
pub trait EmulatorCore {
    fn load_rom(&mut self, rom: &[u8], persisted: Option<&BatteryState>) -> Result<CartridgeMetadata, CoreError>;
    fn reset(&mut self) -> Result<(), CoreError>;
    fn run_cycles(&mut self, cycle_budget: u32) -> Result<RunOutcome, CoreError>;
    fn set_input(&mut self, source: InputSourceId, state: JoypadState);
    fn clear_input_source(&mut self, source: InputSourceId);
    fn take_frame(&mut self) -> Option<Frame>;
    fn drain_audio(&mut self) -> AudioBatch;
    fn battery_state(&self) -> Option<BatteryState>;
}
```

`Frame` owns exactly `160 * 144 * 4` RGBA bytes and a monotonically increasing sequence. `AudioBatch` contains interleaved `f32` stereo samples and a sample rate. `InputSourceId` is an opaque `u64` newtype; `JoypadState` uses one bit per valid button and has no invalid representable buttons. `CoreError` distinguishes invalid ROM, unsupported CGB-only cartridge, unsupported mapper, not-loaded, and internal invariant violation.

- [ ] **Step 4: Prove isolation and commit**

```bash
cargo fmt --all --check
cargo clippy -p gb-core --all-targets --no-default-features -- -D warnings
cargo test -p gb-core --no-default-features
cargo tree -p gb-core
git add crates/gb-core Cargo.toml Cargo.lock
git commit -m "feat(core): define isolated emulator contracts"
```

Expected: `cargo tree -p gb-core` contains only `gb-core`; no Tauri, Tokio, serde, filesystem, network, or audio dependency appears.

### Task 4: Mirror protocol and session boundaries in gb-network

**Files:**
- Create: `crates/gb-network/Cargo.toml`.
- Create: `crates/gb-network/src/lib.rs`.
- Create: `crates/gb-network/src/message.rs`.
- Create: `crates/gb-network/src/session.rs`.
- Create: `crates/gb-network/tests/protocol_fixtures.rs`.
- Create: `crates/gb-network/tests/session_contract.rs`.

**Interfaces:**
- Consumes: `packages/protocol/fixtures/protocol-v1.json`, serde/serde_json, and `gb-core::InputSourceId`.
- Produces: Rust `ClientMessage`, `ServerMessage`, `SessionId`, `SessionToken`, `ControllerConnectionId`, and `ControllerEvent` types that match protocol v1.

- [ ] **Step 1: Write failing fixture and session tests**

Deserialize every valid fixture into the matching Rust enum, reject every invalid fixture, and round-trip messages without losing fields. Test that `SessionToken` redacts its value in `Debug`, that `ControllerEvent::Disconnected` carries the connection's `InputSourceId`, and that session IDs/tokens cannot be empty.

- [ ] **Step 2: Run and confirm failure**

Run `cargo test -p gb-network`.

Expected: FAIL because the crate and message/session types do not exist.

- [ ] **Step 3: Implement the mirror types only**

Use `#[serde(tag = "type", rename_all = "kebab-case")]` enums that exactly mirror the TypeScript fixtures. Do not open sockets or spawn Tokio tasks. Tokens are constructed from non-empty strings in tests but their future random generation remains a PED-39 responsibility.

- [ ] **Step 4: Verify and commit**

```bash
cargo fmt --all --check
cargo clippy -p gb-network --all-targets -- -D warnings
cargo test -p gb-network
git add crates/gb-network Cargo.toml Cargo.lock
git commit -m "feat(network): define remote session contracts"
```

Expected: TypeScript and Rust consume the same fixture successfully.

### Task 5: Bootstrap the React/Tauri desktop shell

**Files:**
- Create: `apps/desktop/package.json`.
- Create: `apps/desktop/tsconfig.json`.
- Create: `apps/desktop/vite.config.ts`.
- Create: `apps/desktop/vitest.config.ts`.
- Create: `apps/desktop/index.html`.
- Create: `apps/desktop/src/main.tsx`.
- Create: `apps/desktop/src/app/App.tsx`.
- Create: `apps/desktop/src/app/router.tsx`.
- Create: `apps/desktop/src/app/providers.tsx`.
- Create: `apps/desktop/src/pages/EmulatorPage.tsx`.
- Create: `apps/desktop/src/styles.css`.
- Create: `apps/desktop/src/test/setup.ts`.
- Create: `apps/desktop/src/pages/EmulatorPage.test.tsx`.
- Create: `apps/desktop/src-tauri/Cargo.toml`.
- Create: `apps/desktop/src-tauri/build.rs`.
- Create: `apps/desktop/src-tauri/tauri.conf.json`.
- Create: `apps/desktop/src-tauri/capabilities/default.json`.
- Create: `apps/desktop/src-tauri/src/lib.rs`.
- Create: `apps/desktop/src-tauri/src/main.rs`.
- Create: `apps/desktop/src-tauri/src/contracts.rs`.

**Interfaces:**
- Consumes: `gb-core`, `gb-network`, and `@gameboy/protocol` without implementing their feature behavior.
- Produces: a buildable Tauri application, typed `foundation_status` command, React provider/router shell, and minimal empty-state UI.

- [ ] **Step 1: Install exact desktop dependencies**

Use the current official Tauri/Vite React TypeScript setup and `pnpm --filter @gameboy/desktop add --save-exact ...` for React, Tauri API, TanStack Router/Query and devtools, Zod, React Hook Form, dayjs, Tabler icons, Tailwind, shadcn prerequisites, Vitest, Testing Library, jsdom, TypeScript, Vite, and Tauri CLI. Do not add Axios because the desktop has no REST API.

- [ ] **Step 2: Write the failing React shell test**

Render `EmulatorPage` and assert the visible heading `Game Boy`, status `No ROM loaded`, button `Open ROM`, and remote text `Mobile controller is off`. The foundation-only controls may be disabled in PED-34, but they must be accessible by role and name.

- [ ] **Step 3: Run and confirm failure**

Run `pnpm --filter @gameboy/desktop test`.

Expected: FAIL because the page and test setup do not exist.

- [ ] **Step 4: Implement the minimal React shell**

Configure `@/*`, Tailwind, shadcn zinc tokens, TanStack Router with a single `/` route, QueryClient providers, development-only devtools, and a focused `EmulatorPage`. Keep route files thin and page code under `src/pages`.

- [ ] **Step 5: Write the failing Tauri contract test**

In `src-tauri/src/contracts.rs`, test that `foundation_status()` returns serializable fields `protocol_version: "v1"`, `screen_width: 160`, `screen_height: 144`, and `remote_controller_limit: 1`.

- [ ] **Step 6: Implement the Tauri foundation command**

Expose `foundation_status` via `#[tauri::command]` and `generate_handler!`. The Tauri crate depends on `gb-core` and `gb-network` by relative path but does not start an emulator or server. Configure the Vite dev URL and build output, a unique bundle identifier, Apple Silicon/Windows-friendly icons generated by the official Tauri path, and minimum capabilities required only for the foundation command.

- [ ] **Step 7: Verify desktop and commit**

```bash
pnpm --filter @gameboy/desktop lint
pnpm --filter @gameboy/desktop typecheck
pnpm --filter @gameboy/desktop test
pnpm --filter @gameboy/desktop build
cargo test -p gameboy-desktop
cargo check -p gameboy-desktop
git add apps/desktop package.json pnpm-lock.yaml Cargo.lock
git commit -m "feat(desktop): bootstrap React and Tauri shell"
```

Expected: web build and Rust checks pass; starting `pnpm --filter @gameboy/desktop tauri dev` opens the shell on a machine with Tauri prerequisites.

### Task 6: Bootstrap the independent mobile controller shell

**Files:**
- Create: `apps/remote-controller/package.json`.
- Create: `apps/remote-controller/tsconfig.json`.
- Create: `apps/remote-controller/vite.config.ts`.
- Create: `apps/remote-controller/vitest.config.ts`.
- Create: `apps/remote-controller/index.html`.
- Create: `apps/remote-controller/public/manifest.webmanifest`.
- Create: `apps/remote-controller/src/main.tsx`.
- Create: `apps/remote-controller/src/app/App.tsx`.
- Create: `apps/remote-controller/src/app/router.tsx`.
- Create: `apps/remote-controller/src/app/providers.tsx`.
- Create: `apps/remote-controller/src/pages/ControllerPage.tsx`.
- Create: `apps/remote-controller/src/styles.css`.
- Create: `apps/remote-controller/src/test/setup.ts`.
- Create: `apps/remote-controller/src/pages/ControllerPage.test.tsx`.

**Interfaces:**
- Consumes: `@gameboy/protocol` and no desktop source files.
- Produces: independently buildable responsive web shell that displays protocol version and disconnected state.

- [ ] **Step 1: Install exact mobile dependencies**

Install the same React/Vite baseline as desktop, excluding Tauri packages. Include TanStack Router/Query and devtools, Zod, React Hook Form, dayjs, Tabler icons, Tailwind, shadcn prerequisites, Vitest, Testing Library, jsdom, and a workspace dependency on `@gameboy/protocol`. Do not add Axios; PED-38 uses WebSocket directly.

- [ ] **Step 2: Write the failing mobile shell test**

Render `ControllerPage` and assert heading `Game Boy Controller`, connection state `Disconnected`, all eight button labels, and protocol text `Protocol v1`.

- [ ] **Step 3: Run and confirm failure**

Run `pnpm --filter @gameboy/remote-controller test`.

Expected: FAIL because the controller page does not exist.

- [ ] **Step 4: Implement the minimal responsive shell**

Add a single `/` route, providers/devtools, portrait/landscape CSS regions, safe-area padding, accessible button elements, PWA manifest metadata, and disconnected styling. Buttons remain inert in PED-34; PED-38 owns pointer handling and connection state.

- [ ] **Step 5: Prove independent build and commit**

```bash
pnpm --filter @gameboy/remote-controller lint
pnpm --filter @gameboy/remote-controller typecheck
pnpm --filter @gameboy/remote-controller test
pnpm --filter @gameboy/remote-controller build
git add apps/remote-controller package.json pnpm-lock.yaml
git commit -m "feat(remote): bootstrap mobile controller shell"
```

Expected: build output contains `index.html` and `manifest.webmanifest`; no import reaches into `apps/desktop`.

### Task 7: Document frozen boundaries and test strategy

**Files:**
- Create: `docs/architecture/workspace.md`.
- Create: `docs/architecture/core-contracts.md`.
- Create: `docs/architecture/protocol-v1.md`.
- Create: `docs/architecture/runtime-boundaries.md`.
- Create: `docs/testing/strategy.md`.
- Create: `docs/testing/rom-assets.md`.
- Modify: `README.md`.

**Interfaces:**
- Consumes: implemented Task 1-6 manifests, public Rust types, TypeScript schemas, and fixtures.
- Produces: exact documentation future sub-issue plans can cite without guessing ownership or signatures.

- [ ] **Step 1: Write boundary documentation from actual code**

Document module dependency direction, every `EmulatorCore` method and protocol message, ownership table, high-frequency framebuffer/audio constraint, input-source union behavior, persistence boundary, session security boundary, and the process for changing a frozen contract.

- [ ] **Step 2: Write the test strategy**

Specify unit/integration/ROM/UI layers; fixture locations; licensing rules; how Blargg and Mooneye revisions/checksums will be pinned later; and root commands that must pass before a sub-issue moves to Done. Explicitly state that no commercial ROM enters the repository.

- [ ] **Step 3: Verify documentation matches code**

Run:

```bash
rg "trait EmulatorCore|enum ClientMessage|enum ServerMessage|PROTOCOL_VERSION" crates packages
rg "EmulatorCore|ClientMessage|ServerMessage|v1" docs/architecture docs/testing
```

Expected: every public name documented exists with the same spelling; no document claims implemented emulation behavior.

- [ ] **Step 4: Commit the architecture record**

```bash
git add docs README.md
git commit -m "docs: record emulator contracts and test strategy"
```

### Task 8: Add cross-platform CI and finish PED-34 verification

**Files:**
- Create: `.github/workflows/ci.yml`.
- Modify: `README.md`.

**Interfaces:**
- Consumes: all PED-34 workspace commands.
- Produces: Linux-fast checks plus native macOS Apple Silicon and Windows x64 compile/build evidence.

- [ ] **Step 1: Add CI jobs**

Create jobs with pinned major action versions and minimum permissions:

- `quality`: Ubuntu, Node 24.20.0, Corepack/pnpm cache, Rust stable, `pnpm install --frozen-lockfile`, lint, typecheck, TypeScript tests, Rust fmt/clippy/tests;
- `desktop-macos`: Apple Silicon runner, production frontend build and `cargo check -p gameboy-desktop`;
- `desktop-windows`: Windows x64 runner with MSVC Rust toolchain, production frontend build and `cargo check -p gameboy-desktop`.

Installer signing and notarization are explicitly excluded from PED-34; the workflow proves compilation only.

- [ ] **Step 2: Run the full local verification**

```bash
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check -p gb-core --no-default-features
cargo metadata --no-deps --format-version 1
git diff --check
```

Expected: every command exits `0`; Cargo lists all three workspace crates; no lint or formatting warning remains.

- [ ] **Step 3: Perform the PED-34 acceptance review**

Confirm:

- Tauri shell starts on macOS Apple Silicon;
- `gb-core` compiles alone and has no forbidden dependency;
- framebuffer, audio, input, lifecycle, battery, clock, and fault contracts exist;
- mobile shell builds independently;
- protocol v1 fixtures pass in TypeScript and Rust;
- test and ownership strategy is documented;
- no CPU, PPU, APU, listening server, or ROM feature leaked into the foundation.

- [ ] **Step 4: Commit CI and review fixes**

```bash
git add .github/workflows/ci.yml README.md
git commit -m "ci: verify emulator foundation across targets"
```

After the commit, move PED-34 to `In Review`. Resolve all review findings with focused tests and separate commits, rerun Step 2, then move PED-34 to `Done`.
