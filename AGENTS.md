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

## Linear, branches, and pull requests

Linear is the source of truth for implementation work. Keep issue statuses and dependency relations aligned with the actual state of the repository.

- For a simple task, create one issue containing the goal, scope, acceptance criteria, and implementation plan. Create a branch from `develop` whose name is exactly the Linear identifier, such as `PED-32`, and open its pull request back to `develop`.
- For a complex task, create one parent orchestration issue plus dependency-linked sub-issues. Create the parent branch from `develop`; create each sub-issue branch from the parent branch and merge its pull request back into the parent branch in dependency order. Open the final parent pull request to `develop`.
- Use multiple agents only for unblocked work with independent ownership. Do not let agents edit the same manifests, lockfiles, workflows, or integration paths concurrently.
- Pull request titles must use `<Linear ID>: <issue title>`, for example `PED-32: Pockiva — Game Boy`.
- Do not push directly to `develop` or `main`. Feature and orchestration pull requests use squash merge. Release pull requests from `develop` to `main` use a merge commit.
- A merge from `develop` to `main` is a release boundary. The release pull request must include a SemVer increase and consistent versions across the shipped Tauri, Cargo, and desktop package metadata.
- Do not publish tags or releases manually unless the automated release workflow is unavailable and the user explicitly approves a recovery procedure.

## Verification

Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `cargo test --workspace --all-features` before review.

## ROM assets

Never commit or silently download commercial ROMs. Developer scripts may fetch explicitly selected redistributable test ROMs only after their revision, source, license, and checksum are recorded.
