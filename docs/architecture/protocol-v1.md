# Controller Protocol v1

The canonical TypeScript schema lives in `packages/protocol/src/messages.ts`; `PROTOCOL_VERSION` is the literal string `v1`. Its public unions are `ClientMessage` and `ServerMessage`, parsed by `parseClientMessage` and `parseServerMessage`. Traffic is JSON. `packages/protocol/fixtures/protocol-v1.json` is the shared compatibility fixture consumed by TypeScript and Rust tests.

## Client messages

| Type | Fields | Meaning |
| --- | --- | --- |
| `hello` | `version: "v1"`, non-empty `token` | Starts the handshake. This is the only client message containing a token. |
| `button-down` | `button`, integer `sequence >= 0` | Marks one valid button pressed. |
| `button-up` | `button`, integer `sequence >= 0` | Marks one valid button released. |
| `state-sync` | unique `buttons[]`, integer `sequence >= 0` | Replaces the controller's complete pressed-button snapshot. |
| `ping` | integer `sequence >= 0` | Heartbeat request. |

Valid buttons are `up`, `down`, `left`, `right`, `a`, `b`, `start`, and `select`.

## Server messages

| Type | Fields | Meaning |
| --- | --- | --- |
| `welcome` | `version: "v1"`, non-empty `controllerId` | Accepts the controller. |
| `rejected` | `reason` | Rejects the handshake/message. |
| `pong` | integer `sequence >= 0` | Echoes heartbeat sequencing. |
| `controller-disconnected` | none | Reports that the controller connection ended. |

Rejection reasons are `invalid-token`, `unsupported-version`, `controller-already-connected`, and `malformed-message`. Schemas reject unknown message variants, invalid fields, negative/fractional sequences, duplicate state buttons, empty handshake identifiers, and unexpected token fields.

`crates/gb-network/src/message.rs` mirrors these variants with internally tagged Serde enums. Its fixture test deserializes and round-trips every valid canonical object and rejects every invalid object. `SessionToken` redacts secrets in `Debug`; `SessionId`, `SessionToken`, and `ControllerConnectionId` reject empty values. `ControllerEvent::Disconnected` always carries the `InputSourceId` that a future transport must clear.

PED-34 defines no socket, route, rate limiter, token generator, or live connection lifecycle. Those implementations must preserve this schema and session boundary.
