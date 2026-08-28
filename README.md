# Pockiva

Pockiva is a desktop emulator with separate Game Boy and Game Boy Advance modes, targeting macOS Apple Silicon and Windows x64. The current implementation provides the Game Boy mode with keyboard input and an optional local-network mobile controller. Emulation and network behavior are developed behind explicit contracts.

## Prerequisites

- Node.js 24.20.0
- pnpm through Corepack
- Rust stable with `rustfmt` and `clippy`
- Tauri 2 platform prerequisites for desktop builds

## Workspace commands

```sh
pnpm install
pnpm lint
pnpm typecheck
pnpm test
pnpm build
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Use `pnpm dev:desktop` for the Tauri application and `pnpm dev:remote` for the browser controller after installing platform prerequisites.

No ROM files are bundled. Do not add commercial ROMs to this repository.

## Foundation documentation

- [Workspace architecture](docs/architecture/workspace.md)
- [Core contracts](docs/architecture/core-contracts.md)
- [Controller protocol v1](docs/architecture/protocol-v1.md)
- [Runtime boundaries](docs/architecture/runtime-boundaries.md)
- [Testing strategy](docs/testing/strategy.md)
- [ROM asset policy](docs/testing/rom-assets.md)

The workspace now includes emulation, live ROM lifecycle, desktop audio/video output, and a LAN controller server. See the architecture documents above for the current runtime and protocol boundaries.

## Continuous integration

CI runs the complete quality/test suite on Ubuntu and compiles the desktop frontend and Tauri crate on native macOS Apple Silicon and Windows x64 runners.

## Development and release flow

- `develop` is the default integration branch.
- Work branches use the exact Linear issue identifier, such as `PED-32`, and merge into `develop` through pull requests.
- `main` is reserved for releases. Only a merged pull request from `develop` may trigger the release workflow.
- The release version follows SemVer and must be kept consistent in the Tauri configuration, desktop package, and Cargo package metadata.
- The automated release produces signed Tauri updater artifacts for macOS Apple Silicon and Windows x64 and publishes them through GitHub Releases only after both platform builds succeed.

Tauri updater artifacts are cryptographically signed, but Apple notarization and Windows Authenticode signing are not configured yet. Until those platform signatures are added, macOS Gatekeeper and Windows SmartScreen may warn users about downloaded installers.

No ROM files, proprietary BIOS files, updater private keys, or signing passwords are bundled in this repository.
