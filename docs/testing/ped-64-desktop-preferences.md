# PED-64 desktop preferences and controls evidence

Validation date: 2026-08-28

## Implemented behavior

- Volume from 0% to 100% and mute are applied to the active audio output without restarting the ROM.
- Volume, mute, and display scale are stored in the local Tauri `settings.json` store and restored on startup.
- Missing settings use safe defaults; malformed settings are replaced with those defaults.
- Display choices are 1x, 2x, 3x, 4x, and fit-to-window. The canvas remains 160 x 144 and CSS scaling preserves its aspect ratio and pixelated rendering.
- ROM actions are grouped in a visible command section. Pause and resume share one state-aware action; invalid lifecycle actions are disabled.
- Desktop shortcuts are Command/Ctrl+O for opening a ROM, Space for pause/resume, and Shift+Command/Ctrl+R for restart. Editable controls and the keyboard-mapping dialog do not trigger them.
- Native application-menu duplicates were intentionally omitted in this delivery. The visible command section keeps discovery and state synchronization in one cross-platform surface.

## Automated verification

The following commands passed from the repository root:

```text
pnpm lint
pnpm typecheck
pnpm test
pnpm build
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p gb-core --no-default-features
```

Results:

- JavaScript/TypeScript: 225 tests passed across protocol, desktop, and remote-controller packages.
- Rust: 253 tests passed and 25 were ignored across the full workspace.
- Minimal `gb-core`: 147 tests passed and 23 were ignored.

## macOS Apple Silicon

- The production Tauri build completed for `aarch64-apple-darwin`.
- Both `Game Boy Emulator.app` and the Apple Silicon DMG were generated.
- Visual inspection confirmed the grouped ROM controls, audio controls, display scale selector, and remote-controller section render correctly.
- The application loaded an existing persisted `fit` display setting from the local store.

## Windows x64

Native Windows x64 packaging and end-to-end persistence/audio validation remain pending. They belong to the final cross-platform validation in PED-40; PED-64 should remain in progress until that acceptance criterion is exercised.
