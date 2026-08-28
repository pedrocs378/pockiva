# PED-65 remote controller joystick evidence

Validation date: 2026-08-28

## Implemented behavior

- The remote controller offers an accessible `D-pad` / `Joystick` selector.
- The selected directional mode is validated, stored locally, and restored after the controller page reloads.
- The fixed joystick uses a 24% dead zone and eight digital sectors: four cardinal directions and four diagonals.
- Pointer ownership supports simultaneous directional and action inputs without releasing unrelated A/B touches when the directional mode changes.
- Pointer cancellation, disconnect, page hide, and mode changes release owned directions so inputs cannot remain stuck.
- Landscape layouts enlarge the D-pad, joystick, and A/B controls while preserving safe-area padding.
- Short landscape layouts below `22rem` retain a minimum 9rem directional surface and remain inside the viewport.
- Protocol v1 is unchanged; joystick movement is translated into the existing digital button transitions.

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

- JavaScript/TypeScript: 268 tests passed: 7 protocol, 170 desktop, and 91 remote-controller tests.
- Rust: 253 tests passed and 25 were ignored across the full workspace.
- Minimal `gb-core`: 147 tests passed and 23 were ignored.
- Both desktop and remote-controller production builds completed.

## Browser viewport validation

The local controller page was inspected in the Codex in-app browser at these viewport sizes:

- 390 x 844 portrait
- 844 x 390 landscape
- 667 x 375 compact landscape
- 568 x 320 short landscape

For both directional modes, the document dimensions matched the viewport and the controller shell, directional surface, action controls, and menu controls stayed within it. The short landscape joystick rendered at 144 x 144 pixels, matching the 9rem minimum. Selecting `Joystick` and reloading the page restored the joystick selection.

## Remaining physical-device acceptance

Real-phone validation remains pending. Acceptance should cover rotation between portrait and landscape, eight joystick directions, simultaneous joystick plus A/B input, safe areas, disconnect/reconnect cleanup, and preference restoration after reopening the controller.
