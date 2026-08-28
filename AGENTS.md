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
- Do not push directly to `develop` or `main`.
- Every task, sub-issue, and parent orchestration pull request whose base branch is `develop` must always be integrated with a squash merge. Never use a merge commit or rebase merge for a pull request targeting `develop`.
- Every release pull request from `develop` to `main` must always be integrated with a regular merge commit. Never squash or rebase a `develop` to `main` release pull request.
- A merge from `develop` to `main` is a release boundary. The release pull request must include a SemVer increase and consistent versions across the shipped Tauri, Cargo, and desktop package metadata.
- Do not publish tags or releases manually unless the automated release workflow is unavailable and the user explicitly approves a recovery procedure.

### SemVer policy

- Choose one version bump per delivery into `develop`, not one per commit. A simple task owns its bump; for a complex task, the final parent pull request owns the single bump and child pull requests into the parent branch do not bump independently.
- Use `patch` for bug fixes, documentation, CI changes, internal refactors, and small backward-compatible adjustments. Use `minor` for new backward-compatible user-facing capabilities. Use `major` for incompatible public behavior after `1.0.0`.
- While Pockiva remains in `0.x`, incompatible behavior advances `minor`; `1.0.0` marks the first stable public contract. Compatibility and user-visible behavior, not change size alone, determine the bump.
- Keep `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/package.json`, `[workspace.package]` in `Cargo.toml`, and inherited local packages in `Cargo.lock` consistent by using the root `version:bump` command.
- If a future `develop` to `main` release pull request has no explicit bump, release automation chooses `patch`; it never replaces an explicit valid `minor` or `major` bump.

## Verification

Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `cargo test --workspace --all-features` before review.

## ROM assets

Never commit or silently download commercial ROMs. Developer scripts may fetch explicitly selected redistributable test ROMs only after their revision, source, license, and checksum are recorded.
