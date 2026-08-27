# Contributor Guide

## Workspace ownership

- `crates/gb-core`: platform-independent emulation contracts and implementation. It must not depend on Tauri, React, filesystem, network, or platform-audio APIs.
- `crates/gb-network`: local controller protocol and session transport. It consumes public `gb-core` input identifiers and never mutates emulator internals directly.
- `packages/protocol`: canonical TypeScript wire protocol. Rust mirrors must be verified against its committed fixtures.
- `apps/desktop`: React desktop UI and Tauri platform adapter.
- `apps/remote-controller`: independent browser controller; it may consume `@gameboy/protocol` but must not import desktop source files.

## Dependency and contract rules

Work follows PED-32's dependency order: foundation first, then core, desktop, and remote lanes; integration follows those lanes. Public `gb-core` and protocol v1 contract changes require coordinated TypeScript fixture, Rust mirror, tests, and architecture-document updates in one change. Keep protocol version `v1` until an intentionally versioned migration is approved.

All JavaScript dependencies are exact versions. Use Node.js 24.20.0, pnpm, TypeScript, Biome, and the root aliases and import ordering. Rust uses stable, workspace lints, and the committed `Cargo.lock`.

## Verification

Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `cargo test --workspace --all-features` before review.

## ROM assets

Never commit or silently download commercial ROMs. Developer scripts may fetch explicitly selected redistributable test ROMs only after their revision, source, license, and checksum are recorded.
