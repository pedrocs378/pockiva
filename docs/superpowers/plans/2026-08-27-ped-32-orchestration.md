# PED-32 Orchestration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the PED-32 Game Boy emulator MVP by executing its Linear sub-issues in dependency order while keeping code ownership and Linear status synchronized.

**Architecture:** PED-32 is an orchestration plan, not one merge-sized implementation. PED-34 freezes workspace and contract boundaries; each later sub-issue receives its own executable plan after its dependencies are complete, and independent modules are delegated in parallel only when their file ownership does not overlap.

**Tech Stack:** Rust, Cargo workspaces, Tauri 2, React, TypeScript, Vite, pnpm, Vitest, Biome, WebSocket, GitHub Actions

**Spec:** `docs/superpowers/specs/2026-08-27-game-boy-emulator-design.md`

## Global Constraints

- Target macOS Apple Silicon and Windows x64.
- Emulate DMG hardware; accept dual-mode DMG/GBC cartridges through DMG compatibility mode and reject CGB-only cartridges.
- Support ROM-only, MBC1, MBC3, and MBC5 cartridges with battery-backed persistence.
- Keep `gb-core` independent of React, Tauri, filesystem, networking, and platform audio.
- Keyboard remains a complete control path; one optional mobile controller may connect or disconnect without restarting the ROM.
- Use Blargg, Mooneye, and a redistributable homebrew ROM for validation; never bundle commercial ROMs.
- Use TypeScript, pnpm, exact dependency versions, Biome, and Node.js `24.20.0` LTS.
- Move a Linear issue only when its real implementation state changes.

---

### Task 1: Establish the foundation through PED-34

**Files:**
- Create/modify only the files enumerated by `docs/superpowers/plans/2026-08-27-ped-34-foundation.md`.

**Interfaces:**
- Consumes: approved PED-32 design specification.
- Produces: compiling Cargo/pnpm workspaces, protocol v1 schemas and fixtures, Rust core/runtime contracts, React/Tauri shells, architecture documentation, and CI baseline.

- [ ] **Step 1: Re-read current Linear blockers and start the parent and foundation issues**

Use the Linear connector to verify that PED-34 has no blocker, then set PED-32 and PED-34 to `In Progress`. Do not change PED-35, PED-36, PED-37, PED-38, PED-39, PED-40, or PED-49 yet.

- [ ] **Step 2: Execute the PED-34 plan**

Follow `docs/superpowers/plans/2026-08-27-ped-34-foundation.md` task-by-task with a review gate after every task.

- [ ] **Step 3: Review PED-34 against its acceptance criteria**

Run:

```bash
rtk pnpm install --frozen-lockfile
rtk pnpm lint
rtk pnpm typecheck
rtk pnpm test
rtk pnpm build
rtk cargo fmt --all --check
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk cargo test --workspace --all-features
rtk cargo check -p gb-core --no-default-features
```

Expected: every command exits `0`; `gb-core` builds without Tauri; both web applications build; protocol fixtures pass in TypeScript and Rust.

- [ ] **Step 4: Synchronize PED-34**

Move PED-34 to `In Review` before review. Move it to `Done` only after review findings are resolved and Step 3 passes. Re-fetch PED-35, PED-37, and PED-38 and verify PED-34 no longer blocks them.

### Task 2: Run Phase 1 with three exclusive owners

**Files:**
- Core owner: `crates/gb-core/**`, `tests/roms/**`, `scripts/fetch-core-test-roms.sh`, `scripts/verify-core-test-roms.sh`, `.gitignore` only for the ROM-download exclusion, `docs/compatibility/core.md`, `docs/architecture/core-contracts.md`, and `docs/testing/rom-assets.md`.
- Desktop owner: `apps/desktop/**` except `apps/desktop/src-tauri/src/remote/**` and shared Cargo manifests.
- Mobile owner: `apps/remote-controller/**`.
- Contract changes: `packages/protocol/**`, public modules in `crates/gb-core/src/contracts/**`, and workspace manifests require coordinator approval.
- Shared lockfiles: the coordinator owns `Cargo.lock`; lane agents may update their own manifests but must not stage or commit `Cargo.lock`. PED-37 owns `pnpm-lock.yaml` in Phase 1 because it is the only lane allowed to add JavaScript dependencies.

**Interfaces:**
- Consumes: frozen PED-34 protocol and core/runtime contracts.
- Produces: PED-35 core execution, PED-37 keyboard-first desktop shell, and PED-38 mobile/PWA pairing client against mocks.

- [ ] **Step 1: Create executable phase plans from the frozen contracts**

Create these plans with exact function signatures and tests copied from the implemented PED-34 contracts:

```text
docs/superpowers/plans/2026-08-27-ped-35-core.md
docs/superpowers/plans/2026-08-27-ped-37-desktop.md
docs/superpowers/plans/2026-08-27-ped-38-mobile-controller.md
```

Each plan must pass the writing-plans self-review before dispatch.

- [ ] **Step 2: Re-fetch blockers and start only unblocked issues**

Use Linear immediately before dispatch. Move PED-35, PED-37, and PED-38 to `In Progress` only if PED-34 is `Done` and their blocker lists are empty. Before starting PED-35, also run the ROM-asset preflight from its plan. If either local-only Blargg ROM is absent, report that external acceptance dependency to the user immediately; implementation may proceed, but PED-35 must remain `In Progress` and no dependent lane may start until the checksum-matched files are supplied and both ignored Blargg tests pass.

- [ ] **Step 3: Dispatch three agents with exclusive ownership**

Assign one agent to each issue. No agent may edit another owner's paths. Shared-contract requests are sent to the coordinator and applied serially after compatibility review.

- [ ] **Step 3a: Reconcile the shared Rust lockfile serially**

After PED-35 and PED-37 have both finished their manifest edits, pause their Rust verification gates. With both manifests present, the coordinator updates `Cargo.lock` once, reviews the complete resolution, runs `rtk cargo check --workspace --all-targets --all-features`, and commits only `Cargo.lock`. The lane agents then resume focused Rust checks; neither lane stages the shared lockfile.

- [ ] **Step 4: Review, verify, and synchronize each issue independently**

For each issue, run its plan's focused checks plus the complete workspace checks. Move that issue to `In Review` while it is under review and to `Done` only when its own acceptance criteria pass. A failure in one lane must not falsely complete another lane.

### Task 3: Run CPU-dependent graphics and audio lanes

**Files:**
- PED-36 owner: `crates/gb-core/src/ppu/**`, `crates/gb-core/tests/ppu/**`, `apps/desktop/src-tauri/src/video/**`, `apps/desktop/src/features/emulator/video/**`.
- PED-49 owner: `crates/gb-core/src/apu/**`, `crates/gb-core/tests/apu/**`, `apps/desktop/src-tauri/src/audio/**`.
- Shared timing/bus files and `apps/desktop/src-tauri/src/emulator/runtime.rs` require coordinator-owned serial integration. PED-49 supplies the tested PCM/pacing adapter; the coordinator performs and commits the runtime hookup after PED-49's isolated implementation is stable and before PED-39 starts.

**Interfaces:**
- Consumes: completed PED-35 machine-cycle, bus, interrupt, framebuffer, and audio contracts.
- Produces: stable frame output and bounded PCM output integrated at the desktop runtime boundary.

- [ ] **Step 1: Create PED-36 and PED-49 executable plans**

Create:

```text
docs/superpowers/plans/2026-08-27-ped-36-ppu.md
docs/superpowers/plans/2026-08-27-ped-49-apu.md
```

The plans must name every shared timing/bus change. If both require the same file, schedule that change serially through the coordinator before parallel work resumes.

- [ ] **Step 2: Re-fetch blockers and dispatch the independent modules**

Start PED-36 and PED-49 only after PED-35 and PED-37 are both `Done`. Move both to `In Progress`, dispatch separate owners, and retain coordinator ownership of shared core and desktop-runtime integration.

- [ ] **Step 3: Integrate shared timing and runtime files serially**

After the isolated PED-36 and PED-49 modules are stable, pause both owners. The coordinator integrates their named shared bus/timing changes and hooks PED-49's tested audio adapter into `apps/desktop/src-tauri/src/emulator/runtime.rs`, then runs the focused core, video, audio, and desktop runtime tests. Finish this serial commit before PED-39 receives runtime ownership.

- [ ] **Step 4: Verify and synchronize both lanes**

Require graphical test ROM evidence for PED-36 and audio register/timing evidence for PED-49. Run the full workspace suite after merging both lanes, then move each through `In Review` to `Done` independently.

### Task 4: Integrate the real remote controller through PED-39

**Files:**
- Modify: `crates/gb-network/**`.
- Modify: `apps/desktop/src-tauri/src/remote/**`.
- Create: `apps/desktop/src-tauri/src/emulator/factory.rs` by extracting the production factory boundary from PED-37's `emulator/runtime.rs` without changing the runtime-facing `CoreFactory` contract.
- Modify: `apps/desktop/src-tauri/src/emulator/runtime.rs` only where the production core factory and remote input source are connected.
- Modify: `apps/desktop/src-tauri/src/lib.rs` to register the production `SystemClock` + `GameBoy` factory.
- Modify: `apps/desktop/src/features/remote-controller/**`.
- Modify: `apps/remote-controller/src/features/session/**` only for integration corrections.
- Modify: `packages/protocol/**` only through a version-compatible coordinator change.

**Interfaces:**
- Consumes: PED-35 concrete `GameBoy` and input source contract, PED-37 injectable desktop lifecycle, PED-38 protocol client.
- Produces: production `SystemClock` + `GameBoy` factory replacing the desktop mock, authenticated local HTTP/WebSocket session, QR pairing, source-aware remote input, heartbeat cleanup, and reconnect behavior.

- [ ] **Step 1: Create the PED-39 executable plan**

Create `docs/superpowers/plans/2026-08-27-ped-39-remote-integration.md` from the actual protocol, runtime commands, and input types.

- [ ] **Step 2: Verify all three blockers and start PED-39**

PED-35, PED-37, and PED-38 must all be `Done`. Move PED-39 to `In Progress` only after re-fetching those statuses.

- [ ] **Step 3: Execute, review, and synchronize PED-39**

Verify that production startup constructs the real `GameBoy` rather than `ContractMockCore`, then verify a real supported ROM reaches core execution, one active mobile client, second-client rejection, invalid/expired token rejection, button down/up, multi-button state, heartbeat timeout, unexpected disconnect cleanup, keyboard coexistence, and ROM continuity. Move through `In Review` to `Done` only after all checks pass.

### Task 5: Consolidate and validate through PED-40

**Files:**
- Create: `docs/compatibility/blargg.md`.
- Create: `docs/compatibility/mooneye.md`.
- Create: `docs/compatibility/homebrew.md`.
- Create: `docs/known-limitations.md`.
- Create: `docs/release/macos-apple-silicon.md`.
- Create: `docs/release/windows-x64.md`.
- Create: `apps/desktop/src-tauri/src/persistence/battery.rs`.
- Create: `apps/desktop/src-tauri/src/persistence/mod.rs`.
- Modify: `apps/desktop/src-tauri/src/emulator/runtime.rs` for persisted-state load, periodic dirty checkpoints, ROM replacement, close, and shutdown flushes.
- Modify implementation files only for defects discovered by PED-40, with a regression test beside each fix.

**Interfaces:**
- Consumes: all completed core, PPU, APU, desktop, mobile, and integration work.
- Produces: cross-platform atomic battery-save persistence, verified MVP evidence, and documented compatibility/platform limitations.

- [ ] **Step 1: Create the PED-40 executable validation plan**

Create `docs/superpowers/plans/2026-08-27-ped-40-validation.md` with the exact ROM asset revisions, expected test signatures, manual scenarios, duration of the prolonged run, platform build commands, and a TDD task for battery saves keyed by ROM identity. The persistence task must cover loading `BatteryState` before execution, format/version validation, corrupt-file preservation, temporary-file + atomic replacement behavior on macOS and Windows, periodic dirty checkpoints, ROM replacement, close, and application shutdown.

- [ ] **Step 2: Verify blockers and start PED-40**

PED-36, PED-37, PED-39, and PED-49 must be `Done`. Move PED-40 to `In Progress` only after Linear confirms every dependency.

- [ ] **Step 3: Execute validation and repair regressions**

Implement and verify battery persistence before compatibility validation. Every later code correction receives a failing regression test before implementation. Repeat the relevant ROM, workspace, lifecycle, save/reload, and platform checks after each fix.

- [ ] **Step 4: Complete PED-40**

Move PED-40 to `In Review`, conduct final review, rerun the full suite and both platform builds, then move it to `Done` only when all PED-40 criteria and evidence documents are complete.

### Task 6: Close the orchestrator truthfully

**Files:**
- Modify: `README.md` with supported hardware, ROM policy, development commands, and known limitations links.
- Modify: `docs/known-limitations.md` with the final compatibility statement.

**Interfaces:**
- Consumes: final PED-40 evidence and statuses for all mandatory sub-issues.
- Produces: release-ready repository state and completed PED-32.

- [ ] **Step 1: Re-fetch every mandatory sub-issue**

Confirm PED-34, PED-35, PED-36, PED-37, PED-38, PED-39, PED-40, and PED-49 are all `Done`. If any issue is not Done, keep PED-32 `In Progress`.

- [ ] **Step 2: Run the final clean verification**

Run:

```bash
rtk pnpm install --frozen-lockfile
rtk pnpm lint
rtk pnpm typecheck
rtk pnpm test
rtk pnpm build
rtk cargo fmt --all --check
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk cargo test --workspace --all-features
rtk pnpm --filter @gameboy/desktop tauri build
```

Expected: all commands exit `0` on macOS Apple Silicon; the Windows x64 CI build and installer job are green for the same commit.

- [ ] **Step 3: Complete PED-32**

Move PED-32 to `Done` only after Step 1 and Step 2 succeed and the final review has no release-blocking finding.
