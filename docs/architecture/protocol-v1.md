# Controller Protocol v1

The canonical TypeScript schema lives in `packages/protocol/src/messages.ts`; `PROTOCOL_VERSION` is the literal string `v1`. Its public unions are `ClientMessage` and `ServerMessage`, parsed by `parseClientMessage` and `parseServerMessage`. Traffic is JSON. `packages/protocol/fixtures/protocol-v1.json` is the shared compatibility fixture consumed by TypeScript and Rust tests.

## Client messages

| Type | Fields | Meaning |
| --- | --- | --- |
| `hello` | `version: "v1"`, non-empty `token` | Starts the handshake. This is the only client message containing a token. |
| `button-down` | `button`, safe integer `sequence >= 0` | Marks one valid button pressed. |
| `button-up` | `button`, safe integer `sequence >= 0` | Marks one valid button released. |
| `state-sync` | unique `buttons[]`, safe integer `sequence >= 0` | Replaces the controller's complete pressed-button snapshot. |
| `ping` | safe integer `sequence >= 0` | Heartbeat request. |

Valid buttons are `up`, `down`, `left`, `right`, `a`, `b`, `start`, and `select`.

## Server messages

| Type | Fields | Meaning |
| --- | --- | --- |
| `welcome` | `version: "v1"`, non-empty `controllerId` | Accepts the controller. |
| `rejected` | `reason` | Rejects the handshake/message. |
| `pong` | safe integer `sequence >= 0` | Echoes heartbeat sequencing. |
| `controller-disconnected` | none | Reports that the controller connection ended. |

Rejection reasons are `invalid-token`, `unsupported-version`, `controller-already-connected`, and `malformed-message`. Sequence values are restricted to `0..=9_007_199_254_740_991` (`Number.MAX_SAFE_INTEGER`); Rust represents them with the validated `Sequence` newtype. Schemas reject unknown message variants, invalid fields, out-of-range or fractional sequences, duplicate state buttons, empty handshake identifiers, and unexpected token fields.

`crates/gb-network/src/message.rs` mirrors these variants with internally tagged Serde enums. Its fixture test deserializes and round-trips every valid canonical object and rejects every invalid object. `SessionToken` redacts secrets in `Debug`; `SessionId`, `SessionToken`, and `ControllerConnectionId` reject empty values. Authenticated transport activity is represented internally as `ControllerEvent::Connected`, `ControllerEvent::Message`, and `ControllerEvent::Disconnected`, each carrying the connection and remote `InputSourceId`; these are not wire messages.

## Transport rules

- The browser loads the controller assets over HTTP from the pairing origin and opens `/controller` on that same host and port using `ws:` or `wss:` as appropriate.
- The first WebSocket text frame must be `hello`. The server allows five seconds for this handshake and rejects binary or malformed frames.
- After `welcome`, `button-down`, `button-up`, `state-sync`, and `ping` share one contiguous sequence. The successor of `Number.MAX_SAFE_INTEGER` is `0`, so wraparound remains safe in JavaScript.
- Application text is limited to 4,096 bytes before JSON parsing. The WebSocket decoder permits one additional sentinel byte so a 4,097-byte payload can receive `rejected: malformed-message`; payloads beyond that tightly bounded sentinel close at the transport layer.
- Production applies a token bucket of 240 accepted messages per second with a burst capacity of 64.
- Every valid authenticated message refreshes an absolute server deadline of 18 seconds. `ping` participates in the same sequence and receives a matching `pong`.
- If an `Origin` header is present, it must exactly match the advertised HTTP pairing origin.
- Closing, timing out, rejecting, ending, or shutting down a connection emits at most one internal disconnect event so the desktop runtime can clear only the remote input source.

These transport rules implement the existing schema. They add no message, field, rejection reason, or protocol version; the canonical protocol remains `v1` and its committed fixtures are unchanged.
