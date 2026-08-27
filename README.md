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
