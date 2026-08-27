# Testing Strategy

Every change starts with the narrowest behavior test and ends with root verification.

## Layers

- Rust unit tests cover focused core behavior as it is implemented: registers, instructions, memory, timers, interrupts, input, mappers, PPU, and APU.
- Rust integration tests cover public contracts, deterministic clocks, lifecycle, persistence data, source-aware input, disconnect cleanup, frame delivery, and bounded audio.
- Protocol compatibility tests use `packages/protocol/fixtures/protocol-v1.json` from both Vitest and `gb-network` integration tests.
- React unit tests use Vitest, Testing Library, and jsdom for stable visible states and accessible controls.
- ROM harness tests later run explicitly pinned Blargg and Mooneye cases with bounded cycles and explicit pass/fail signals.
- Platform checks compile the frontend and Tauri adapter on macOS Apple Silicon and Windows x64. Final end-to-end validation uses a legally redistributable homebrew ROM.

## Required local checks

```sh
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

A sub-issue is not ready for Done until its focused tests, applicable root checks, documented acceptance criteria, and review findings pass. A platform-specific check that cannot run locally must be recorded and covered by CI rather than assumed.

Test failures must identify bounded time/cycle limits and exact fixtures. Compatibility claims must name the test-ROM revision and checksum; a passing shell/build is not evidence of emulation compatibility.
