# Game Boy Emulator

Foundation for a desktop Game Boy emulator targeting macOS Apple Silicon and Windows x64, plus an optional local-network mobile controller. Emulation and network behavior are intentionally developed behind explicit contracts.

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

The current foundation compiles shells and freezes contracts; it does not yet implement emulation, live ROM lifecycle, or a listening controller server.

## Continuous integration

CI runs the complete quality/test suite on Ubuntu and compiles the desktop frontend and Tauri crate on native macOS Apple Silicon and Windows x64 runners. Installer signing, notarization, and release packaging are outside PED-34; these jobs provide compilation evidence only.
