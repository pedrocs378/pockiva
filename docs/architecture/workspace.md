# Workspace Architecture

The repository is one pnpm workspace and one Cargo workspace. The dependency direction is deliberately one-way:

```text
packages/protocol ───────────────► apps/remote-controller
        │                         apps/desktop (React)
        │
        └── shared JSON fixture ─► crates/gb-network ─► apps/desktop/src-tauri
                                         │                    │
crates/gb-core ──────────────────────────┘────────────────────┘
```

No core or network crate imports application code.

| Area | Owns | Must not own |
| --- | --- | --- |
| `packages/protocol` | Canonical protocol v1 schemas, inferred TypeScript types, fixtures | WebSocket lifecycle, UI state |
| `crates/gb-core` | Platform-independent emulator contracts and, in later issues, emulation | Filesystem, network, Tauri, React, platform audio |
| `crates/gb-network` | Rust wire mirror and session identifiers/events | A listening server in PED-34, emulator mutation |
| `apps/desktop` | React shell and Tauri platform adapter | CPU/PPU/APU implementation |
| `apps/remote-controller` | Independent mobile web shell | Desktop source imports, native application behavior |

The delivery order is foundation, independent core/desktop/remote lanes, integration, and final compatibility validation. JavaScript dependencies are exact versions, protocol fixtures are committed, and application-producing Cargo workspaces commit `Cargo.lock`.

Changes to `EmulatorCore` or protocol v1 are frozen-boundary changes. Update public types, all tests, the canonical JSON fixture, both language mirrors where relevant, and these architecture documents together. A wire-incompatible change requires an intentionally versioned protocol migration rather than silently changing `v1`.
