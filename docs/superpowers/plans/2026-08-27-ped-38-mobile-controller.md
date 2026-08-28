# PED-38 Mobile Controller Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver an independently buildable mobile/PWA Game Boy controller that pairs from a tokenized QR URL, connects to a simulated protocol-v1 session, sends correct multi-touch input, survives transient disconnects, and works in portrait and landscape mobile browsers.

**Architecture:** Keep browser transport, protocol/session state, pointer tracking, React lifecycle, and rendering in separate focused modules under `apps/remote-controller`. The production client derives a WebSocket endpoint from the QR URL and speaks only the frozen `@gameboy/protocol` v1 schema; deterministic unit/component tests inject an in-memory mock server and fake scheduler, so PED-38 does not implement the real desktop server owned by PED-39.

**Tech Stack:** Node.js 24.20.0, pnpm, TypeScript 7, React 19, Vite 8, TanStack Router, Tailwind CSS 4, Vitest 4, Testing Library, native Pointer Events, native WebSocket, Web App Manifest, `@gameboy/protocol` v1

**Spec:** `docs/superpowers/specs/2026-08-27-game-boy-emulator-design.md`

## Global Constraints

- Modify only `apps/remote-controller/**`; do not edit `apps/desktop`, `crates/gb-network`, `crates/gb-core`, or `packages/protocol` while executing PED-38.
- Import canonical wire types and parsers only from `@gameboy/protocol`; never import desktop application source.
- Keep `PROTOCOL_VERSION` equal to the literal `v1`. A protocol change requires coordinated TypeScript fixture, Rust mirror, tests, and architecture-document changes and is outside this issue unless separately approved.
- The QR pairing URL contract is `http(s)://<local-host>:<port>/?token=<url-encoded-token>`; the client derives `ws(s)://<same-host>:<same-port>/controller` and sends the token only in the `hello` message.
- The real HTTP/WebSocket listener, QR rendering, token generation, one-controller enforcement, rate limiting, and emulator input integration belong to PED-39. PED-38 uses only a browser transport adapter plus an in-memory test server.
- Valid buttons are exactly `up`, `down`, `left`, `right`, `a`, `b`, `start`, and `select`.
- Every post-handshake client message uses a safe-integer sequence in `0..=Number.MAX_SAFE_INTEGER`; after the maximum, the client wraps to `0`.
- Heartbeat timing is exact: schedule the first `ping` `5_000 ms` after `welcome`, schedule each later `ping` `5_000 ms` after the preceding matching `pong`, require that matching `pong` within `12_000 ms` of its `ping`, and retry unexpected disconnects after `0`, `500`, `1_000`, `2_000`, then `5_000 ms`. After those five failed retries, show `server-unavailable` until the user retries manually.
- Explicit disconnect never auto-reconnects. Unexpected close, heartbeat timeout, and `controller-disconnected` do auto-reconnect while the page is active.
- A successful `welcome` always sends one complete `state-sync` before later deltas, including after reconnection.
- On pointer cancel/lost capture, document hide, `pagehide`, explicit disconnect, or React unmount, clear local pressed state and send `state-sync: []` when the socket is connected.
- Prevent gameplay scroll, selection, pinch/double-tap zoom, pull-to-refresh, tap highlight, and long-press context menus on the controller surface without globally disabling browser zoom through `user-scalable=no`.
- Support both portrait and landscape with safe-area insets. Native installation and offline operation are not completion requirements.
- Add no dependencies. Preserve exact versions, Biome formatting/import order, `@/*` aliases, and the existing React/TanStack Router shell.
- Tests must not open a port, use the network, or depend on wall-clock time.

## File Map

| File | Responsibility |
| --- | --- |
| `apps/remote-controller/src/constants/controller.ts` | Canonical visual/button order, labels, heartbeat, and retry constants. |
| `apps/remote-controller/src/features/session/pairing.ts` | Parse the QR URL and derive the same-origin `/controller` WebSocket URL. |
| `apps/remote-controller/src/features/session/transport.ts` | Narrow transport contract and native browser WebSocket adapter. |
| `apps/remote-controller/src/features/session/controller-session.ts` | Protocol-v1 state machine, sequences, heartbeat, retries, state synchronization, and status mapping. |
| `apps/remote-controller/src/features/session/use-controller-session.ts` | React ownership/subscription/cleanup for one `ControllerSession`. |
| `apps/remote-controller/src/features/controller/pointer-button-tracker.ts` | Pointer-ID-to-button ownership and reference-counted multi-touch transitions. |
| `apps/remote-controller/src/features/controller/use-controller-input.ts` | React pressed-state bridge plus visibility/page lifecycle cleanup. |
| `apps/remote-controller/src/features/controller/ControllerButton.tsx` | Accessible pointer-capturing button with pressed feedback state. |
| `apps/remote-controller/src/test/mock-controller-server.ts` | In-memory simulated protocol-v1 server used only by tests. |
| `apps/remote-controller/src/pages/ControllerPage.tsx` | Compose pairing, session status/actions, D-Pad, action/menu controls, and input hook. |
| `apps/remote-controller/src/styles.css` | Safe-area, gesture suppression, pressed feedback, and portrait/landscape layouts. |
| `apps/remote-controller/public/manifest.webmanifest` | Standalone, orientation-flexible PWA metadata. |

---

### Task 1: Freeze the button and QR URL contracts

**Files:**
- Create: `apps/remote-controller/src/constants/controller.ts`.
- Create: `apps/remote-controller/src/features/session/pairing.ts`.
- Create: `apps/remote-controller/src/features/session/pairing.test.ts`.

**Interfaces:**
- Consumes: `Button` from `@gameboy/protocol` and a standard `URL`.
- Produces: `BUTTON_ORDER`, `D_PAD_BUTTONS`, `ACTION_BUTTONS`, `MENU_BUTTONS`, `BUTTON_LABELS`, `HEARTBEAT_INTERVAL_MS`, `HEARTBEAT_TIMEOUT_MS`, `RECONNECT_DELAYS_MS`, `PairingConfig`, `PairingResult`, and `parsePairingUrl(url: URL): PairingResult`.

- [ ] **Step 1: Write failing pairing tests**

Create `pairing.test.ts` with these exact behaviors:

```ts
import { describe, expect, it } from 'vitest'
import { parsePairingUrl } from './pairing'

describe('parsePairingUrl', () => {
  it('derives an unencrypted local controller socket from a QR URL', () => {
    expect(parsePairingUrl(new URL('http://192.168.1.23:7421/?token=pairing%20token'))).toEqual({
      status: 'ready',
      config: {
        token: 'pairing token',
        socketUrl: 'ws://192.168.1.23:7421/controller'
      }
    })
  })

  it('derives wss from an https QR URL and ignores unrelated query values', () => {
    expect(parsePairingUrl(new URL('https://gb.local/play?theme=dark&token=abc'))).toEqual({
      status: 'ready',
      config: { token: 'abc', socketUrl: 'wss://gb.local/controller' }
    })
  })

  it.each(['http://gb.local/', 'http://gb.local/?token=', 'http://gb.local/?token=%20%20']) (
    'rejects a missing or blank token in %s',
    (value) => expect(parsePairingUrl(new URL(value))).toEqual({ status: 'missing-token' })
  )

  it('rejects origins that cannot map to WebSocket', () => {
    expect(parsePairingUrl(new URL('file:///controller.html?token=abc'))).toEqual({ status: 'invalid-url' })
  })
})
```

- [ ] **Step 2: Run the focused test and verify red**

Run:

```bash
rtk pnpm --filter @gameboy/remote-controller test -- src/features/session/pairing.test.ts
```

Expected: FAIL because `./pairing` does not exist.

- [ ] **Step 3: Implement constants and pairing parsing**

Create `controller.ts`:

```ts
import type { Button } from '@gameboy/protocol'

export const BUTTON_ORDER = ['up', 'down', 'left', 'right', 'a', 'b', 'start', 'select'] as const satisfies readonly Button[]
export const D_PAD_BUTTONS = ['up', 'left', 'right', 'down'] as const satisfies readonly Button[]
export const ACTION_BUTTONS = ['b', 'a'] as const satisfies readonly Button[]
export const MENU_BUTTONS = ['select', 'start'] as const satisfies readonly Button[]

export const BUTTON_LABELS: Record<Button, string> = {
  up: 'Up',
  down: 'Down',
  left: 'Left',
  right: 'Right',
  a: 'A',
  b: 'B',
  start: 'Start',
  select: 'Select'
}

export const HEARTBEAT_INTERVAL_MS = 5_000
export const HEARTBEAT_TIMEOUT_MS = 12_000
export const RECONNECT_DELAYS_MS = [0, 500, 1_000, 2_000, 5_000] as const
```

Create `pairing.ts`:

```ts
export type PairingConfig = { token: string; socketUrl: string }
export type PairingResult = { status: 'ready'; config: PairingConfig } | { status: 'missing-token' | 'invalid-url' }

export const parsePairingUrl = (url: URL): PairingResult => {
  const token = url.searchParams.get('token')?.trim()
  if (!token) return { status: 'missing-token' }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') return { status: 'invalid-url' }

  const socketUrl = new URL('/controller', url)
  socketUrl.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
  socketUrl.search = ''
  socketUrl.hash = ''
  return { status: 'ready', config: { token, socketUrl: socketUrl.toString() } }
}
```

- [ ] **Step 4: Verify green and commit**

Run:

```bash
rtk pnpm --filter @gameboy/remote-controller test -- src/features/session/pairing.test.ts
rtk pnpm --filter @gameboy/remote-controller typecheck
```

Expected: both commands exit `0`; the tests prove the QR URL never puts the token in the WebSocket URL.

```bash
rtk git add apps/remote-controller/src/constants/controller.ts apps/remote-controller/src/features/session/pairing.ts apps/remote-controller/src/features/session/pairing.test.ts
rtk git commit -m "feat(remote): define controller pairing URL"
```

### Task 2: Isolate native WebSocket behind a testable transport

**Files:**
- Create: `apps/remote-controller/src/features/session/transport.ts`.
- Create: `apps/remote-controller/src/features/session/transport.test.ts`.

**Interfaces:**
- Consumes: browser `WebSocket` supplied through `WebSocketConstructor`.
- Produces: `TransportHandlers`, `SessionConnection`, `SessionTransport`, `WebSocketConstructor`, and `createWebSocketTransport(WebSocketImpl?: WebSocketConstructor): SessionTransport`.

- [ ] **Step 1: Write the failing adapter test**

```ts
import { describe, expect, it, vi } from 'vitest'
import { createWebSocketTransport, type WebSocketConstructor } from './transport'

it('forwards socket lifecycle and delegates send and close', () => {
  const listeners = new Map<string, (event: Event | MessageEvent<string>) => void>()
  const socket = {
    addEventListener: vi.fn((type: string, listener: (event: Event | MessageEvent<string>) => void) => listeners.set(type, listener)),
    removeEventListener: vi.fn(),
    send: vi.fn(),
    close: vi.fn()
  }
  const WebSocketImpl = vi.fn(() => socket) as unknown as WebSocketConstructor
  const handlers = { onOpen: vi.fn(), onMessage: vi.fn(), onClose: vi.fn(), onError: vi.fn() }

  const connection = createWebSocketTransport(WebSocketImpl).connect('ws://gb.local/controller', handlers)
  listeners.get('open')?.(new Event('open'))
  listeners.get('message')?.(new MessageEvent('message', { data: '{"type":"pong","sequence":1}' }))
  connection.send('hello')
  connection.close(1000, 'done')

  expect(WebSocketImpl).toHaveBeenCalledWith('ws://gb.local/controller')
  expect(handlers.onOpen).toHaveBeenCalledOnce()
  expect(handlers.onMessage).toHaveBeenCalledWith('{"type":"pong","sequence":1}')
  expect(socket.send).toHaveBeenCalledWith('hello')
  expect(socket.close).toHaveBeenCalledWith(1000, 'done')
})
```

- [ ] **Step 2: Run and verify red**

Run `rtk pnpm --filter @gameboy/remote-controller test -- src/features/session/transport.test.ts`.

Expected: FAIL because `transport.ts` does not exist.

- [ ] **Step 3: Implement the narrow adapter**

```ts
export type TransportHandlers = {
  onOpen: () => void
  onMessage: (data: string) => void
  onClose: () => void
  onError: () => void
}

export type SessionConnection = {
  send: (data: string) => void
  close: (code?: number, reason?: string) => void
}

export type SessionTransport = {
  connect: (url: string, handlers: TransportHandlers) => SessionConnection
}

export type WebSocketConstructor = new (url: string) => Pick<
  WebSocket,
  'addEventListener' | 'removeEventListener' | 'send' | 'close'
>

export const createWebSocketTransport = (
  WebSocketImpl: WebSocketConstructor = WebSocket
): SessionTransport => ({
  connect: (url, handlers) => {
    const socket = new WebSocketImpl(url)
    const onOpen = () => handlers.onOpen()
    const onMessage = (event: MessageEvent<unknown>) => {
      if (typeof event.data === 'string') handlers.onMessage(event.data)
      else handlers.onError()
    }
    const onClose = () => handlers.onClose()
    const onError = () => handlers.onError()
    socket.addEventListener('open', onOpen)
    socket.addEventListener('message', onMessage as EventListener)
    socket.addEventListener('close', onClose)
    socket.addEventListener('error', onError)
    return {
      send: (data) => socket.send(data),
      close: (code, reason) => socket.close(code, reason)
    }
  }
})
```

Do not add URL rewriting, protocol parsing, retry logic, or a mock mode to this adapter.

- [ ] **Step 4: Verify and commit**

```bash
rtk pnpm --filter @gameboy/remote-controller test -- src/features/session/transport.test.ts
rtk pnpm --filter @gameboy/remote-controller typecheck
rtk git add apps/remote-controller/src/features/session/transport.ts apps/remote-controller/src/features/session/transport.test.ts
rtk git commit -m "feat(remote): isolate controller websocket transport"
```

Expected: tests and type checking pass with no network access.

### Task 3: Implement protocol-v1 handshake and terminal states against a mock server

**Files:**
- Create: `apps/remote-controller/src/features/session/controller-session.ts`.
- Create: `apps/remote-controller/src/features/session/controller-session.test.ts`.
- Create: `apps/remote-controller/src/test/mock-controller-server.ts`.

**Interfaces:**
- Consumes: `PairingConfig`, `SessionTransport`, `parseServerMessage`, `PROTOCOL_VERSION`, `MAX_SAFE_SEQUENCE`, and timing constants.
- Produces: `SessionStatus`, `SessionSnapshot`, `SessionScheduler`, `ControllerSessionOptions`, and class `ControllerSession` with `connect(): void`, `disconnect(): void`, `setButton(button: Button, pressed: boolean): void`, `syncButtons(buttons: readonly Button[]): void`, `getSnapshot(): SessionSnapshot`, and `subscribe(listener: () => void): () => void`.

- [ ] **Step 1: Create the in-memory server and failing handshake tests**

Implement the test double with this public surface:

```ts
export type MockControllerServerOptions = {
  validToken?: string
  rejectionReason?: 'invalid-token' | 'unsupported-version' | 'controller-already-connected' | 'malformed-message'
  autoPong?: boolean
}

export class MockControllerServer {
  readonly receivedMessages: ClientMessage[] = []
  connectionCount = 0
  autoPong: boolean

  constructor(options: MockControllerServerOptions = {})
  createTransport(): SessionTransport
  dropConnection(): void
  sendRaw(data: string): void
}
```

Defaults are `validToken: 'pairing-token'`, `autoPong: true`, and controller ID `controller-1`. `createTransport()` must create no real socket. It queues `onOpen`, parses each client payload with `parseClientMessage`, responds to a matching-token `hello` with `welcome`, responds to a non-matching token with `rejected: invalid-token`, responds to configured rejection modes with their exact `rejected` reason, optionally echoes `ping` as `pong`, and lets `dropConnection()` invoke the active `onClose`. Then write these tests in `controller-session.test.ts`:

```ts
it('sends hello, accepts welcome, and exposes a stable connected snapshot', async () => {
  const server = new MockControllerServer({ validToken: 'pairing-token' })
  const session = createSession(server)
  session.connect()
  await flushMicrotasks()

  expect(server.receivedMessages[0]).toEqual({ type: 'hello', version: 'v1', token: 'pairing-token' })
  expect(session.getSnapshot()).toMatchObject({ status: 'connected', controllerId: 'controller-1' })
})

it.each([
  ['invalid-token', 'expired-token'],
  ['unsupported-version', 'incompatible-protocol'],
  ['controller-already-connected', 'controller-in-use'],
  ['malformed-message', 'server-unavailable']
] as const)('maps %s rejection to %s without retrying', async (reason, status) => {
  const server = new MockControllerServer({ rejectionReason: reason })
  const session = createSession(server)
  session.connect()
  await flushMicrotasks()
  expect(session.getSnapshot().status).toBe(status)
  expect(server.connectionCount).toBe(1)
})

it('treats malformed JSON and an invalid server schema as incompatible protocol', async () => {
  const server = new MockControllerServer()
  const session = createSession(server)
  session.connect()
  await flushMicrotasks()
  server.sendRaw('{')
  expect(session.getSnapshot().status).toBe('incompatible-protocol')
})
```

The shared test helper is exact:

```ts
const sessions = new Set<ControllerSession>()
const flushMicrotasks = async () => await Promise.resolve()
const createSession = (server: MockControllerServer, overrides: Partial<ControllerSessionOptions> = {}) => {
  const session = new ControllerSession({
    pairing: { token: 'pairing-token', socketUrl: 'ws://gb.local/controller' },
    transport: server.createTransport(),
    ...overrides
  })
  sessions.add(session)
  return session
}

afterEach(() => {
  for (const session of sessions) session.disconnect()
  sessions.clear()
  vi.clearAllTimers()
  vi.useRealTimers()
})
```

- [ ] **Step 2: Run and verify red**

Run `rtk pnpm --filter @gameboy/remote-controller test -- src/features/session/controller-session.test.ts`.

Expected: FAIL because `ControllerSession` does not exist.

- [ ] **Step 3: Implement the state and handshake core**

Use these exact types:

```ts
export type SessionStatus =
  | 'connecting'
  | 'connected'
  | 'disconnected'
  | 'expired-token'
  | 'incompatible-protocol'
  | 'controller-in-use'
  | 'server-unavailable'

export type SessionSnapshot = {
  status: SessionStatus
  controllerId: string | null
  pressedButtons: ReadonlySet<Button>
  reconnectAttempt: number
}

export type SessionScheduler = {
  setTimeout: (callback: () => void, delayMs: number) => ReturnType<typeof setTimeout>
  clearTimeout: (timer: ReturnType<typeof setTimeout>) => void
}

export type ControllerSessionOptions = {
  pairing: PairingConfig
  transport: SessionTransport
  scheduler?: SessionScheduler
  heartbeatIntervalMs?: number
  heartbeatTimeoutMs?: number
  reconnectDelaysMs?: readonly number[]
  initialSequence?: number
}
```

Start with snapshot `{ status: 'disconnected', controllerId: null, pressedButtons: new Set(), reconnectAttempt: 0 }`. `connect()` sets `connecting`, opens exactly one connection, and sends:

```ts
{ type: 'hello', version: PROTOCOL_VERSION, token: pairing.token }
```

Parse every incoming payload with `JSON.parse` followed by `parseServerMessage`. On `welcome`, require its schema-proven `version: 'v1'`, set `connected`, record `controllerId`, reset retry count, and publish a new immutable snapshot object. On `rejected`, map the four reasons exactly as asserted above, close with code `1000`, and suppress retries. On invalid JSON/schema, set `incompatible-protocol`, close, and suppress retries. Use a connection generation counter so callbacks from an old connection cannot mutate a newer one.

Implement subscriptions as:

```ts
subscribe = (listener: () => void) => {
  this.listeners.add(listener)
  return () => this.listeners.delete(listener)
}

getSnapshot = () => this.snapshot
```

- [ ] **Step 4: Verify handshake behavior and commit**

```bash
rtk pnpm --filter @gameboy/remote-controller test -- src/features/session/controller-session.test.ts
rtk pnpm --filter @gameboy/remote-controller typecheck
rtk git add apps/remote-controller/src/features/session/controller-session.ts apps/remote-controller/src/features/session/controller-session.test.ts apps/remote-controller/src/test/mock-controller-server.ts
rtk git commit -m "feat(remote): implement simulated controller handshake"
```

Expected: all handshake/rejection/parser tests pass; no real server or port exists.

### Task 4: Add sequenced input, state synchronization, heartbeat, and reconnection

**Files:**
- Modify: `apps/remote-controller/src/features/session/controller-session.ts`.
- Modify: `apps/remote-controller/src/features/session/controller-session.test.ts`.
- Modify: `apps/remote-controller/src/test/mock-controller-server.ts`.

**Interfaces:**
- Consumes: Task 3 state machine and mock server.
- Produces: protocol-valid button deltas, `state-sync` on every welcome, heartbeat timeout recovery, bounded retry behavior, and manual disconnect/retry semantics.

- [ ] **Step 1: Add failing input and reconnection tests**

```ts
it('sends initial state-sync then simultaneous button deltas with monotonic sequences', async () => {
  const server = new MockControllerServer()
  const session = createSession(server)
  session.connect()
  await flushMicrotasks()
  session.setButton('up', true)
  session.setButton('a', true)
  session.setButton('up', false)

  expect(server.receivedMessages.slice(1)).toEqual([
    { type: 'state-sync', buttons: [], sequence: 0 },
    { type: 'button-down', button: 'up', sequence: 1 },
    { type: 'button-down', button: 'a', sequence: 2 },
    { type: 'button-up', button: 'up', sequence: 3 }
  ])
  expect([...session.getSnapshot().pressedButtons]).toEqual(['a'])
})

it('retains desired input while offline and state-syncs it after reconnect', async () => {
  vi.useFakeTimers()
  const server = new MockControllerServer()
  const session = createSession(server)
  session.connect()
  await flushMicrotasks()
  session.setButton('left', true)
  server.dropConnection()
  expect(session.getSnapshot().status).toBe('connecting')
  session.setButton('b', true)
  await vi.advanceTimersByTimeAsync(0)

  expect(server.connectionCount).toBe(2)
  expect(server.receivedMessages.at(-1)).toEqual({
    type: 'state-sync', buttons: ['left', 'b'], sequence: 2
  })
})

it('sends ping and reconnects after a missing pong deadline', async () => {
  vi.useFakeTimers()
  const server = new MockControllerServer({ autoPong: false })
  const session = createSession(server)
  session.connect()
  await flushMicrotasks()
  await vi.advanceTimersByTimeAsync(5_000)
  expect(server.receivedMessages.at(-1)).toEqual({ type: 'ping', sequence: 1 })
  await vi.advanceTimersByTimeAsync(12_000)
  await vi.advanceTimersByTimeAsync(0)
  expect(server.connectionCount).toBe(2)
})

it('schedules each ping five seconds after welcome or the matching pong', async () => {
  vi.useFakeTimers()
  const server = new MockControllerServer({ autoPong: true })
  const session = createSession(server)
  session.connect()
  await flushMicrotasks()

  await vi.advanceTimersByTimeAsync(4_999)
  expect(server.receivedMessages.filter(({ type }) => type === 'ping')).toHaveLength(0)
  await vi.advanceTimersByTimeAsync(1)
  expect(server.receivedMessages.filter(({ type }) => type === 'ping')).toEqual([
    { type: 'ping', sequence: 1 }
  ])
  await vi.advanceTimersByTimeAsync(4_999)
  expect(server.receivedMessages.filter(({ type }) => type === 'ping')).toHaveLength(1)
  await vi.advanceTimersByTimeAsync(1)
  expect(server.receivedMessages.filter(({ type }) => type === 'ping')).toEqual([
    { type: 'ping', sequence: 1 },
    { type: 'ping', sequence: 2 }
  ])
  expect(session.getSnapshot().status).toBe('connected')
  expect(server.connectionCount).toBe(1)
})

it('reconnects when the server reports controller-disconnected', async () => {
  vi.useFakeTimers()
  const server = new MockControllerServer()
  const session = createSession(server)
  session.connect()
  await flushMicrotasks()
  server.sendRaw(JSON.stringify({ type: 'controller-disconnected' }))
  await vi.advanceTimersByTimeAsync(0)
  expect(server.connectionCount).toBe(2)
  expect(session.getSnapshot().status).toBe('connected')
})

it('manual disconnect releases all input and never reconnects', async () => {
  vi.useFakeTimers()
  const server = new MockControllerServer()
  const session = createSession(server)
  session.connect()
  await flushMicrotasks()
  session.setButton('a', true)
  session.disconnect()
  await vi.runOnlyPendingTimersAsync()

  expect(server.receivedMessages.at(-1)).toMatchObject({ type: 'state-sync', buttons: [] })
  expect(session.getSnapshot()).toMatchObject({ status: 'disconnected', controllerId: null })
  expect(server.connectionCount).toBe(1)
})

it('wraps sequences after Number.MAX_SAFE_INTEGER', async () => {
  const server = new MockControllerServer()
  const session = createSession(server, { initialSequence: Number.MAX_SAFE_INTEGER })
  session.connect()
  await flushMicrotasks()
  session.setButton('a', true)
  expect(server.receivedMessages.slice(-2)).toEqual([
    { type: 'state-sync', buttons: [], sequence: Number.MAX_SAFE_INTEGER },
    { type: 'button-down', button: 'a', sequence: 0 }
  ])
})

it('stops after the five configured retries and allows a manual retry', async () => {
  vi.useFakeTimers()
  const server = new MockControllerServer({ failConnections: true })
  const session = createSession(server)
  session.connect()
  await vi.runAllTimersAsync()
  expect(session.getSnapshot().status).toBe('server-unavailable')
  expect(server.connectionCount).toBe(6)
  server.failConnections = false
  session.connect()
  await flushMicrotasks()
  expect(session.getSnapshot().status).toBe('connected')
})
```

Extend `MockControllerServerOptions` with `failConnections?: boolean`, expose mutable `failConnections`, and make a failed connection queue `onError` followed by `onClose` once.

- [ ] **Step 2: Run the tests and verify the missing behavior**

Run `rtk pnpm --filter @gameboy/remote-controller test -- src/features/session/controller-session.test.ts`.

Expected: FAIL on the first absent `state-sync`/input method or the first retry assertion.

- [ ] **Step 3: Implement sequence and input rules**

Maintain a desired `Set<Button>`. `setButton(button, true)` is idempotent if already pressed; otherwise add and send `button-down` only while connected. `setButton(button, false)` is likewise idempotent and sends `button-up`. `syncButtons(buttons)` replaces the desired set, sorts it through `BUTTON_ORDER`, publishes it immediately, and sends one `state-sync` while connected.

Use this sequence helper:

```ts
private takeSequence = () => {
  const sequence = this.nextSequence
  this.nextSequence = sequence === MAX_SAFE_SEQUENCE ? 0 : sequence + 1
  return sequence
}
```

On every `welcome`, send `state-sync` with the current desired buttons, then schedule the first ping for exactly `heartbeatIntervalMs` later. Do not reset `nextSequence` across reconnects.

`disconnect()` must call `syncButtons([])` while the connection is still writable, set the manual-disconnect flag, cancel every timer, close with code `1000` and reason `client-disconnect`, clear the controller ID, and publish `disconnected`. This makes direct calls and React unmount cleanup safe without relying on a component to release first.

- [ ] **Step 4: Implement heartbeat and bounded reconnect**

Use one next-ping timeout, one pong-deadline timeout, and one reconnect timeout; do not use `setInterval`. On `welcome`, arm the next-ping timeout for `heartbeatIntervalMs`. When it fires, send one `ping`, record its sequence as the only awaited pong, and arm the deadline for `heartbeatTimeoutMs`; do not arm another ping while that pong is outstanding. After the matching `pong`, clear the deadline and arm a fresh next-ping timeout for `heartbeatIntervalMs`, so network response time never shortens the five-second quiet period. Ignore stale/wrong-sequence pong messages; their existing deadline must still expire. On heartbeat timeout close with code `4000`, reason `heartbeat-timeout`, then schedule retry. On `controller-disconnected`, close with code `4001`, reason `server-disconnected`, then retry. On transport `error`, close the current connection and let the same idempotent retry path handle the following `close`. On any unexpected `close`, cancel heartbeat timers and consume the next delay. Guard retry scheduling by connection generation plus a `retryTimer !== null` check so `error`+`close` and `controller-disconnected`+`close` consume only one delay. On explicit disconnect or terminal rejection/parser failure, cancel every timer and suppress retry.

The retry transition must publish:

```ts
{
  status: 'connecting',
  controllerId: null,
  pressedButtons: new Set(this.desiredButtons),
  reconnectAttempt: attempt + 1
}
```

After the last delay has been consumed, publish `server-unavailable`. `connect()` called manually from any non-connected terminal/disconnected state resets the retry counter and starts a new attempt.

- [ ] **Step 5: Verify all session behavior and commit**

```bash
rtk pnpm --filter @gameboy/remote-controller test -- src/features/session/controller-session.test.ts
rtk pnpm --filter @gameboy/remote-controller typecheck
rtk pnpm --filter @gameboy/remote-controller lint
rtk git add apps/remote-controller/src/features/session/controller-session.ts apps/remote-controller/src/features/session/controller-session.test.ts apps/remote-controller/src/test/mock-controller-server.ts
rtk git commit -m "feat(remote): synchronize and recover controller sessions"
```

Expected: input ordering, reconnect state-sync, five-seconds-after-pong heartbeat cadence, heartbeat timeout, explicit disconnect, retry exhaustion, and safe sequence wrapping pass under fake timers without pending-timer warnings.

### Task 5: Build reference-counted multi-touch controls with pointer capture

**Files:**
- Create: `apps/remote-controller/src/features/controller/pointer-button-tracker.ts`.
- Create: `apps/remote-controller/src/features/controller/pointer-button-tracker.test.ts`.
- Create: `apps/remote-controller/src/features/controller/ControllerButton.tsx`.
- Create: `apps/remote-controller/src/features/controller/ControllerButton.test.tsx`.
- Modify: `apps/remote-controller/src/test/setup.ts`.

**Interfaces:**
- Consumes: protocol `Button` and React pointer events.
- Produces: `ButtonTransition`, `PointerButtonTracker`, `ControllerButtonProps`, and `ControllerButton`.

- [ ] **Step 1: Write failing pointer tracker tests**

```ts
it('tracks simultaneous pointer ids independently', () => {
  const tracker = new PointerButtonTracker()
  expect(tracker.press(11, 'up')).toEqual({ button: 'up', pressed: true })
  expect(tracker.press(22, 'a')).toEqual({ button: 'a', pressed: true })
  expect(tracker.pressedButtons()).toEqual(['up', 'a'])
  expect(tracker.release(11)).toEqual({ button: 'up', pressed: false })
  expect(tracker.pressedButtons()).toEqual(['a'])
})

it('does not release a button until its final pointer ends', () => {
  const tracker = new PointerButtonTracker()
  expect(tracker.press(1, 'b')).toEqual({ button: 'b', pressed: true })
  expect(tracker.press(2, 'b')).toBeNull()
  expect(tracker.release(1)).toBeNull()
  expect(tracker.release(2)).toEqual({ button: 'b', pressed: false })
})

it('is idempotent for duplicate end events and clears all pointers atomically', () => {
  const tracker = new PointerButtonTracker()
  tracker.press(1, 'left')
  tracker.press(2, 'a')
  expect(tracker.release(99)).toBeNull()
  expect(tracker.clear()).toEqual(['left', 'a'])
  expect(tracker.clear()).toEqual([])
})
```

- [ ] **Step 2: Run the tracker test and verify red**

Run `rtk pnpm --filter @gameboy/remote-controller test -- src/features/controller/pointer-button-tracker.test.ts`.

Expected: FAIL because `PointerButtonTracker` does not exist.

- [ ] **Step 3: Implement pointer ownership**

```ts
import type { Button } from '@gameboy/protocol'
import { BUTTON_ORDER } from '@/constants/controller'

export type ButtonTransition = { button: Button; pressed: boolean }

export class PointerButtonTracker {
  private readonly pointers = new Map<number, Button>()

  press(pointerId: number, button: Button): ButtonTransition | null {
    if (this.pointers.has(pointerId)) return null
    const wasPressed = [...this.pointers.values()].includes(button)
    this.pointers.set(pointerId, button)
    return wasPressed ? null : { button, pressed: true }
  }

  release(pointerId: number): ButtonTransition | null {
    const button = this.pointers.get(pointerId)
    if (!button) return null
    this.pointers.delete(pointerId)
    return [...this.pointers.values()].includes(button) ? null : { button, pressed: false }
  }

  clear(): Button[] {
    const buttons = this.pressedButtons()
    this.pointers.clear()
    return buttons
  }

  pressedButtons(): Button[] {
    const pressed = new Set(this.pointers.values())
    return BUTTON_ORDER.filter((button) => pressed.has(button))
  }
}
```

- [ ] **Step 4: Write failing component tests for pointer capture and cancellation**

First add stable jsdom pointer-capture shims to `src/test/setup.ts`:

```ts
import '@testing-library/jest-dom/vitest'
import { vi } from 'vitest'

Object.defineProperties(HTMLElement.prototype, {
  setPointerCapture: { configurable: true, value: vi.fn() },
  releasePointerCapture: { configurable: true, value: vi.fn() },
  hasPointerCapture: { configurable: true, value: vi.fn(() => true) }
})
```

Then add the component tests:

```tsx
it('captures a primary pointer and reports down/up once', () => {
  const onPress = vi.fn()
  const onRelease = vi.fn()
  render(<ControllerButton button="a" label="A" pressed={false} disabled={false} onPress={onPress} onRelease={onRelease} />)
  const button = screen.getByRole('button', { name: 'A' })
  button.setPointerCapture = vi.fn()
  button.hasPointerCapture = vi.fn(() => true)
  button.releasePointerCapture = vi.fn()
  fireEvent.pointerDown(button, { pointerId: 7, pointerType: 'touch', button: 0 })
  fireEvent.pointerUp(button, { pointerId: 7, pointerType: 'touch', button: 0 })
  expect(button.setPointerCapture).toHaveBeenCalledWith(7)
  expect(onPress).toHaveBeenCalledWith(7, 'a')
  expect(onRelease).toHaveBeenCalledWith(7)
})

it.each(['pointerCancel', 'lostPointerCapture'] as const)('releases on %s', (eventName) => {
  const onRelease = vi.fn()
  render(<ControllerButton button="left" label="Left" pressed disabled={false} onPress={vi.fn()} onRelease={onRelease} />)
  fireEvent[eventName](screen.getByRole('button', { name: 'Left' }), { pointerId: 9 })
  expect(onRelease).toHaveBeenCalledWith(9)
})

it('exposes immediate visual and accessible pressed state', () => {
  render(<ControllerButton button="start" label="Start" pressed disabled={false} onPress={vi.fn()} onRelease={vi.fn()} />)
  expect(screen.getByRole('button', { name: 'Start' })).toHaveAttribute('aria-pressed', 'true')
  expect(screen.getByRole('button', { name: 'Start' })).toHaveAttribute('data-pressed', 'true')
})

it('prevents the long-press context menu', () => {
  render(<ControllerButton button="select" label="Select" pressed={false} disabled={false} onPress={vi.fn()} onRelease={vi.fn()} />)
  const event = createEvent.contextMenu(screen.getByRole('button', { name: 'Select' }), { cancelable: true })
  fireEvent(screen.getByRole('button', { name: 'Select' }), event)
  expect(event.defaultPrevented).toBe(true)
})
```

- [ ] **Step 5: Implement the focused button component**

```tsx
import type { Button } from '@gameboy/protocol'
import type { PointerEvent as ReactPointerEvent } from 'react'
import { Button as UiButton } from '@/components/ui/button'

export type ControllerButtonProps = {
  button: Button
  label: string
  className?: string
  pressed: boolean
  disabled: boolean
  onPress: (pointerId: number, button: Button) => void
  onRelease: (pointerId: number) => void
}

export const ControllerButton = ({ button, label, className, pressed, disabled, onPress, onRelease }: ControllerButtonProps) => {
  const handlePointerDown = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (disabled || (event.pointerType === 'mouse' && event.button !== 0)) return
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    onPress(event.pointerId, button)
  }
  const handlePointerEnd = (event: ReactPointerEvent<HTMLButtonElement>) => {
    event.preventDefault()
    onRelease(event.pointerId)
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
  }

  return (
    <UiButton
      type="button"
      variant="unstyled"
      size="auto"
      className={className}
      disabled={disabled}
      aria-pressed={pressed}
      data-button={button}
      data-pressed={pressed}
      onPointerDown={handlePointerDown}
      onPointerUp={handlePointerEnd}
      onPointerCancel={handlePointerEnd}
      onLostPointerCapture={(event) => onRelease(event.pointerId)}
      onContextMenu={(event) => event.preventDefault()}
    >
      {label}
    </UiButton>
  )
}
```

- [ ] **Step 6: Verify and commit**

```bash
rtk pnpm --filter @gameboy/remote-controller test -- src/features/controller
rtk pnpm --filter @gameboy/remote-controller typecheck
rtk git add apps/remote-controller/src/features/controller apps/remote-controller/src/test/setup.ts
rtk git commit -m "feat(remote): add captured multi-touch controls"
```

Expected: pointer IDs remain independent, duplicate cancel/lost-capture events are harmless through the tracker, and pressed state is visible to assistive technology.

### Task 6: Own session and input lifecycle in React

**Files:**
- Create: `apps/remote-controller/src/features/session/use-controller-session.ts`.
- Create: `apps/remote-controller/src/features/controller/use-controller-input.ts`.
- Create: `apps/remote-controller/src/features/controller/use-controller-input.test.tsx`.

**Interfaces:**
- Consumes: `PairingResult`, `ControllerSession`, `SessionTransport`, `PointerButtonTracker`, React `useEffect`, and `useSyncExternalStore`.
- Produces: `UseControllerSessionResult`, `useControllerSession(pairing, transport)`, `ControllerInputState`, and `useControllerInput(session)`.

- [ ] **Step 1: Write the failing lifecycle test**

Build this exact `renderHook` harness with a connected `ControllerSession`:

```ts
const connectedSession = async () => {
  const server = new MockControllerServer()
  const session = new ControllerSession({
    pairing: { token: 'pairing-token', socketUrl: 'ws://gb.local/controller' },
    transport: server.createTransport()
  })
  session.connect()
  await Promise.resolve()
  return { session, server }
}
```

Then assert:

```tsx
it('clears all pointers and syncs empty state when the document becomes hidden', async () => {
  const { session, server } = await connectedSession()
  const { result } = renderHook(() => useControllerInput(session))
  act(() => {
    result.current.pressPointer(1, 'up')
    result.current.pressPointer(2, 'a')
  })
  expect(result.current.pressedButtons).toEqual(new Set(['up', 'a']))

  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' })
  fireEvent(document, new Event('visibilitychange'))

  expect(result.current.pressedButtons).toEqual(new Set())
  expect(server.receivedMessages.at(-1)).toMatchObject({ type: 'state-sync', buttons: [] })
})

it('performs the same cleanup on pagehide and unmount', async () => {
  const { session, server } = await connectedSession()
  const { result, unmount } = renderHook(() => useControllerInput(session))
  act(() => result.current.pressPointer(4, 'b'))
  fireEvent(window, new Event('pagehide'))
  expect(server.receivedMessages.at(-1)).toMatchObject({ type: 'state-sync', buttons: [] })
  act(() => result.current.pressPointer(5, 'start'))
  unmount()
  expect(server.receivedMessages.at(-1)).toMatchObject({ type: 'state-sync', buttons: [] })
})
```

Restore `document.visibilityState` to `visible` in `afterEach` so tests do not leak state.

- [ ] **Step 2: Run and verify red**

Run `rtk pnpm --filter @gameboy/remote-controller test -- src/features/controller/use-controller-input.test.tsx`.

Expected: FAIL because the input hook does not exist.

- [ ] **Step 3: Implement `useControllerInput`**

Export:

```ts
export type ControllerInputState = {
  pressedButtons: ReadonlySet<Button>
  pressPointer: (pointerId: number, button: Button) => void
  releasePointer: (pointerId: number) => void
  releaseAll: () => void
}
```

Own one `PointerButtonTracker` in a ref. For each non-null tracker transition, update React state immediately and call `session?.setButton(button, pressed)`. `releaseAll()` must call `tracker.clear()`, set an empty React set, and call `session?.syncButtons([])` exactly once. Install `visibilitychange` on `document` and `pagehide` on `window`; call `releaseAll` only when visibility becomes `hidden`, on every `pagehide`, and in effect cleanup. Remove both listeners with the same function references in cleanup.

- [ ] **Step 4: Implement `useControllerSession`**

Use this result contract:

```ts
export type UseControllerSessionResult = {
  session: ControllerSession | null
  snapshot: SessionSnapshot | { status: 'missing-token'; controllerId: null; pressedButtons: ReadonlySet<Button>; reconnectAttempt: 0 }
  connect: () => void
  disconnect: () => void
}
```

Create one `ControllerSession` with `useMemo` only when `pairing.status === 'ready'`. Always call `useSyncExternalStore`; for a missing session pass stable module-level `subscribeMissing = () => () => undefined` and `getMissingSnapshot = () => MISSING_TOKEN_SNAPSHOT`, otherwise pass `session.subscribe` and `session.getSnapshot`. In `useEffect`, call `session?.connect()` and return a cleanup function calling `session?.disconnect()`. Return stable callbacks that delegate to that session. A missing/invalid pairing URL returns the stable missing-token snapshot and no-op actions. This unconditional hook order and cleanup are required to remain safe under React Strict Mode's development setup/cleanup cycle.

- [ ] **Step 5: Verify and commit**

```bash
rtk pnpm --filter @gameboy/remote-controller test -- src/features/controller/use-controller-input.test.tsx
rtk pnpm --filter @gameboy/remote-controller typecheck
rtk pnpm --filter @gameboy/remote-controller lint
rtk git add apps/remote-controller/src/features/session/use-controller-session.ts apps/remote-controller/src/features/controller/use-controller-input.ts apps/remote-controller/src/features/controller/use-controller-input.test.tsx
rtk git commit -m "feat(remote): clean controller input across page lifecycle"
```

Expected: document hide, pagehide, and unmount all produce empty state synchronization and remove their listeners.

### Task 7: Integrate the controller page and all user-visible session states

**Files:**
- Modify: `apps/remote-controller/src/pages/ControllerPage.tsx`.
- Replace: `apps/remote-controller/src/pages/ControllerPage.test.tsx`.

**Interfaces:**
- Consumes: `parsePairingUrl`, `createWebSocketTransport`, `useControllerSession`, `useControllerInput`, `ControllerButton`, button constants, and protocol version.
- Produces: `ControllerPageProps` and a fully interactive page with connect/disconnect/retry controls and clear connection feedback.

- [ ] **Step 1: Write failing page component tests**

Use the injected props:

```ts
export type ControllerPageProps = {
  pairingUrl?: URL
  transport?: SessionTransport
}
```

Replace the foundation test with these scenarios:

```tsx
it('asks for a QR pairing link when the token is missing', () => {
  render(<ControllerPage pairingUrl={new URL('http://gb.local/')} transport={new MockControllerServer().createTransport()} />)
  expect(screen.getByRole('status')).toHaveTextContent('Pairing link required')
  expect(screen.getByText('Scan the QR Code shown by the desktop app.')).toBeVisible()
  expect(screen.getByRole('button', { name: 'A' })).toBeDisabled()
})

it('connects to a simulated session and sends simultaneous A plus Up input', async () => {
  const server = new MockControllerServer()
  render(<ControllerPage pairingUrl={new URL('http://gb.local/?token=pairing-token')} transport={server.createTransport()} />)
  expect(screen.getByRole('status')).toHaveTextContent('Connecting')
  expect(await screen.findByText('Connected')).toBeVisible()
  const up = screen.getByRole('button', { name: 'Up' })
  const a = screen.getByRole('button', { name: 'A' })
  fireEvent.pointerDown(up, { pointerId: 1, pointerType: 'touch', button: 0 })
  fireEvent.pointerDown(a, { pointerId: 2, pointerType: 'touch', button: 0 })
  expect(up).toHaveAttribute('data-pressed', 'true')
  expect(a).toHaveAttribute('data-pressed', 'true')
  expect(server.receivedMessages.slice(-2)).toEqual([
    { type: 'button-down', button: 'up', sequence: 1 },
    { type: 'button-down', button: 'a', sequence: 2 }
  ])
})

it('disconnects, releases input, and reconnects only when requested', async () => {
  const server = new MockControllerServer()
  const user = userEvent.setup()
  render(<ControllerPage pairingUrl={new URL('http://gb.local/?token=pairing-token')} transport={server.createTransport()} />)
  await screen.findByText('Connected')
  fireEvent.pointerDown(screen.getByRole('button', { name: 'B' }), { pointerId: 3, pointerType: 'touch', button: 0 })
  await user.click(screen.getByRole('button', { name: 'Disconnect' }))
  expect(screen.getByRole('status')).toHaveTextContent('Disconnected')
  expect(server.receivedMessages.at(-1)).toMatchObject({ type: 'state-sync', buttons: [] })
  await user.click(screen.getByRole('button', { name: 'Connect' }))
  expect(await screen.findByText('Connected')).toBeVisible()
})

it.each([
  ['invalid-token', 'Pairing link expired'],
  ['unsupported-version', 'Protocol mismatch'],
  ['controller-already-connected', 'Another controller is connected'],
  ['malformed-message', 'Server unavailable']
] as const)('renders the %s rejection clearly', async (rejectionReason, label) => {
  const server = new MockControllerServer({ rejectionReason })
  render(<ControllerPage pairingUrl={new URL('http://gb.local/?token=pairing-token')} transport={server.createTransport()} />)
  expect(await screen.findByText(label)).toBeVisible()
})
```

Reset fake timers, `document.visibilityState`, and mocks in `afterEach`.

- [ ] **Step 2: Run and verify red**

Run `rtk pnpm --filter @gameboy/remote-controller test -- src/pages/ControllerPage.test.tsx`.

Expected: FAIL because `ControllerPage` does not accept injection or render live state.

- [ ] **Step 3: Implement page composition**

At the page boundary:

```tsx
const STATUS_COPY = {
  connecting: 'Connecting',
  connected: 'Connected',
  disconnected: 'Disconnected',
  'missing-token': 'Pairing link required',
  'expired-token': 'Pairing link expired',
  'incompatible-protocol': 'Protocol mismatch',
  'controller-in-use': 'Another controller is connected',
  'server-unavailable': 'Server unavailable'
} as const

const STATUS_HELP = {
  connecting: 'Establishing a local controller session…',
  connected: 'Ready for touch input.',
  disconnected: 'The controller is disconnected.',
  'missing-token': 'Scan the QR Code shown by the desktop app.',
  'expired-token': 'Start a new desktop session and scan its QR Code.',
  'incompatible-protocol': 'Update the desktop and controller to the same version.',
  'controller-in-use': 'Disconnect the active phone before trying again.',
  'server-unavailable': 'Check that the desktop session is still running on this network.'
} as const

export const ControllerPage = ({ pairingUrl, transport }: ControllerPageProps) => {
  const pairing = useMemo(() => parsePairingUrl(pairingUrl ?? new URL(window.location.href)), [pairingUrl])
  const resolvedTransport = useMemo(() => transport ?? createWebSocketTransport(), [transport])
  const controller = useControllerSession(pairing, resolvedTransport)
  const input = useControllerInput(controller.session)
  const status = controller.snapshot.status
  const controlsDisabled = status !== 'connected'
  const disconnect = () => {
    controller.disconnect()
    input.releaseAll()
  }

  return (
    <main className="controller-shell">
      <header className="controller-header">
        <div>
          <p>Remote input</p>
          <h1>Game Boy Controller</h1>
        </div>
        <div className="session-summary">
          <Badge variant="outline" className="connection-state" role="status">
            {status === 'connected' ? <IconWifi aria-hidden="true" size={18} /> : <IconWifiOff aria-hidden="true" size={18} />}
            {STATUS_COPY[status]}
          </Badge>
          <p className="session-help">{STATUS_HELP[status]}</p>
          {status === 'connected' || status === 'connecting' ? (
            <Button type="button" variant="secondary" size="sm" onClick={disconnect}>Disconnect</Button>
          ) : status === 'disconnected' ? (
            <Button type="button" variant="secondary" size="sm" onClick={controller.connect}>Connect</Button>
          ) : status === 'server-unavailable' ? (
            <Button type="button" variant="secondary" size="sm" onClick={controller.connect}>Retry</Button>
          ) : null}
        </div>
      </header>

      <section className="controls" aria-label="Game Boy controls">
        <fieldset className="d-pad">
          <legend className="sr-only">Directional controls</legend>
          {D_PAD_BUTTONS.map((button) => (
            <ControllerButton
              key={button}
              button={button}
              label={BUTTON_LABELS[button]}
              className={`control-button direction ${button}`}
              pressed={input.pressedButtons.has(button)}
              disabled={controlsDisabled}
              onPress={input.pressPointer}
              onRelease={input.releasePointer}
            />
          ))}
        </fieldset>

        <div className="menu-controls">
          {MENU_BUTTONS.map((button) => (
            <ControllerButton
              key={button}
              button={button}
              label={BUTTON_LABELS[button]}
              className="control-button menu"
              pressed={input.pressedButtons.has(button)}
              disabled={controlsDisabled}
              onPress={input.pressPointer}
              onRelease={input.releasePointer}
            />
          ))}
        </div>

        <fieldset className="action-controls">
          <legend className="sr-only">Action controls</legend>
          {ACTION_BUTTONS.map((button) => (
            <ControllerButton
              key={button}
              button={button}
              label={BUTTON_LABELS[button]}
              className={`control-button action action-${button}`}
              pressed={input.pressedButtons.has(button)}
              disabled={controlsDisabled}
              onPress={input.pressPointer}
              onRelease={input.releasePointer}
            />
          ))}
        </fieldset>
      </section>

      <footer>Protocol {PROTOCOL_VERSION}</footer>
    </main>
  )
}
```

Render every control through `ControllerButton`, preserving the existing D-Pad/action/menu class names. The status badge must remain `role="status"` and include an appropriate Tabler icon. Render:

- `Disconnect` for `connecting` or `connected`; its handler calls `controller.disconnect()` first (which synchronizes an empty snapshot before closing) and then `input.releaseAll()` to clear visual pointer state without emitting a second live frame.
- `Connect` for explicit `disconnected`.
- `Retry` for `server-unavailable`.
- no action for missing/expired token, incompatible protocol, or controller-in-use.

All gameplay controls are disabled unless status is `connected`. Keep `Protocol v1` visible. Do not render or generate a QR Code in this app; the desktop session panel in PED-39 generates the URL/QR that this page consumes.

- [ ] **Step 4: Verify and commit**

```bash
rtk pnpm --filter @gameboy/remote-controller test -- src/pages/ControllerPage.test.tsx
rtk pnpm --filter @gameboy/remote-controller typecheck
rtk pnpm --filter @gameboy/remote-controller lint
rtk git add apps/remote-controller/src/pages/ControllerPage.tsx apps/remote-controller/src/pages/ControllerPage.test.tsx
rtk git commit -m "feat(remote): connect mobile controller experience"
```

Expected: all page states, multi-touch feedback, disconnect cleanup, and manual reconnect pass against the simulated server.

### Task 8: Finish mobile gesture protection, PWA metadata, and responsive layouts

**Files:**
- Modify: `apps/remote-controller/src/styles.css`.
- Modify: `apps/remote-controller/public/manifest.webmanifest`.
- Create: `apps/remote-controller/src/mobile-shell.test.ts`.

**Interfaces:**
- Consumes: stable DOM classes/data attributes from Task 7.
- Produces: safe-area-aware portrait/landscape layouts, visual pressed feedback, browser-gesture suppression, and orientation-flexible PWA metadata.

- [ ] **Step 1: Write failing static contract tests**

```ts
import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8')
const manifest = JSON.parse(
  readFileSync(new URL('../public/manifest.webmanifest', import.meta.url), 'utf8')
) as Record<string, unknown>

describe('mobile controller shell', () => {
  it('keeps the PWA standalone and orientation-flexible', () => {
    expect(manifest).toMatchObject({ start_url: '/', scope: '/', display: 'standalone', orientation: 'any' })
  })

  it('suppresses controller gestures and exposes pressed feedback', () => {
    expect(styles).toContain('touch-action: none')
    expect(styles).toContain('overscroll-behavior: none')
    expect(styles).toContain('-webkit-touch-callout: none')
    expect(styles).toContain("[data-pressed='true']")
    expect(html).toContain('viewport-fit=cover')
    expect(html).not.toContain('user-scalable=no')
  })

  it('contains explicit portrait and landscape layouts plus safe areas', () => {
    expect(styles).toContain('@media (orientation: portrait)')
    expect(styles).toContain('@media (orientation: landscape)')
    expect(styles).toContain('env(safe-area-inset-top)')
    expect(styles).toContain('env(safe-area-inset-bottom)')
  })
})
```

- [ ] **Step 2: Run and verify red**

Run `rtk pnpm --filter @gameboy/remote-controller test -- src/mobile-shell.test.ts`.

Expected: FAIL because the manifest lacks `scope`/`orientation` and pressed-state CSS is absent.

- [ ] **Step 3: Update the manifest without claiming offline support**

Use:

```json
{
  "name": "Game Boy Controller",
  "short_name": "GB Controller",
  "description": "Local-network controller for the Game Boy desktop emulator",
  "start_url": "/",
  "scope": "/",
  "display": "standalone",
  "orientation": "any",
  "background_color": "#09090b",
  "theme_color": "#18181b"
}
```

Do not add a service worker, cache policy, or offline claim.

- [ ] **Step 4: Implement the mobile interaction CSS**

Preserve the existing color tokens and layout class names, then add these exact safety and responsive rules:

```css
html,
body,
#root {
  width: 100%;
  min-height: 100%;
  overflow: hidden;
  overscroll-behavior: none;
}

.controller-shell,
.controls {
  user-select: none;
  -webkit-user-select: none;
  -webkit-touch-callout: none;
}

.session-summary {
  display: grid;
  justify-items: end;
  gap: 0.4rem;
  text-align: right;
}

.session-help {
  max-width: 24rem;
  margin: 0;
  color: var(--muted-foreground);
  font-size: 0.75rem;
}

.controls {
  min-height: 0;
  touch-action: none;
}

.control-button {
  min-width: 48px;
  min-height: 48px;
  touch-action: none;
  -webkit-tap-highlight-color: transparent;
  transition: transform 80ms ease, box-shadow 80ms ease, filter 80ms ease;
}

.control-button[data-pressed='true'] {
  transform: translateY(3px) scale(0.96);
  box-shadow: inset 0 3px 8px rgb(0 0 0 / 45%);
  filter: brightness(1.28);
}

@media (orientation: portrait) and (max-width: 44rem) {
  .controls {
    display: grid;
    grid-template: 1fr auto / 1fr 1fr;
    align-content: center;
  }

  .d-pad {
    width: min(42vw, 12rem);
  }

  .action-controls {
    width: min(38vw, 11rem);
  }

  .menu-controls {
    grid-column: 1 / -1;
    grid-row: 2;
    justify-content: center;
  }
}

@media (orientation: landscape) and (max-height: 32rem) {
  .controller-shell {
    gap: 0.5rem;
    padding: max(0.5rem, env(safe-area-inset-top)) max(0.75rem, env(safe-area-inset-right))
      max(0.5rem, env(safe-area-inset-bottom)) max(0.75rem, env(safe-area-inset-left));
  }

  .controls {
    align-self: center;
  }

  .d-pad,
  .action-controls {
    width: min(31dvh, 10rem);
  }

  .menu-controls {
    align-self: center;
  }

  footer {
    display: none;
  }
}
```

Keep the existing default safe-area padding on all four sides using `max(1rem, env(...))`; the landscape override above deliberately reduces only the non-safe-area minimum. Preserve visible focus rings and do not rely on color alone: `aria-pressed` and physical transform provide the second cue.

- [ ] **Step 5: Run focused checks and commit**

```bash
rtk pnpm --filter @gameboy/remote-controller test -- src/mobile-shell.test.ts src/pages/ControllerPage.test.tsx
rtk pnpm --filter @gameboy/remote-controller lint
rtk pnpm --filter @gameboy/remote-controller typecheck
rtk pnpm --filter @gameboy/remote-controller build
rtk git add apps/remote-controller/src/styles.css apps/remote-controller/public/manifest.webmanifest apps/remote-controller/src/mobile-shell.test.ts
rtk git commit -m "feat(remote): finish responsive PWA controller shell"
```

Expected: tests pass and `apps/remote-controller/dist/manifest.webmanifest` contains the updated metadata.

### Task 9: Perform PED-38 acceptance verification and integration handoff

**Files:**
- Modify implementation files under `apps/remote-controller/**` only if a check exposes a defect; add a failing regression test beside every such fix.

**Interfaces:**
- Consumes: all PED-38 tasks.
- Produces: review-ready mobile client and an exact PED-39 handoff contract; no new wire message or real server.

- [ ] **Step 1: Run the complete remote-controller gate**

```bash
rtk pnpm --filter @gameboy/remote-controller lint
rtk pnpm --filter @gameboy/remote-controller typecheck
rtk pnpm --filter @gameboy/remote-controller test
rtk pnpm --filter @gameboy/remote-controller test:coverage
rtk pnpm --filter @gameboy/remote-controller build
```

Expected: every command exits `0`; coverage includes pairing parsing, transport forwarding, all session states, sequence wrapping, five-seconds-after-pong heartbeat cadence, heartbeat timeout, bounded retries, multi-touch reference counting, pointer cancel/lost capture, visibility/pagehide cleanup, and page integration.

- [ ] **Step 2: Prove repository boundaries and protocol stability**

```bash
rtk rg "apps/desktop|src-tauri|gb-network" apps/remote-controller/src apps/remote-controller/package.json; rtk proxy test $? -eq 1
rtk git diff --exit-code bf598fecc320a3464edbf239b35aed90262bb057...HEAD -- packages/protocol crates/gb-network crates/gb-core apps/desktop
rtk git diff --exit-code -- packages/protocol crates/gb-network crates/gb-core apps/desktop
rtk git diff --cached --exit-code -- packages/protocol crates/gb-network crates/gb-core apps/desktop
rtk pnpm --filter @gameboy/protocol test
```

Expected: `rg` exits `1` for the expected no-match case and the following `rtk proxy test` converts only that result to gate success; an actual match (`0`) or search error (`>1`) fails the gate. All three diff commands print nothing and exit `0`: `bf598fecc320a3464edbf239b35aed90262bb057...HEAD` covers committed PED-38 branch changes since the reviewed PED-34 foundation, plain `git diff` covers unstaged changes, and `git diff --cached` covers staged changes. Canonical protocol-v1 tests pass unchanged.

- [ ] **Step 3: Run the complete workspace gate from `AGENTS.md`**

```bash
rtk pnpm lint
rtk pnpm typecheck
rtk pnpm test
rtk pnpm build
rtk cargo fmt --all --check
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk cargo test --workspace --all-features
rtk git diff --check
```

Expected: every command exits `0` with no lint, format, type, test, build, or Rust warning.

- [ ] **Step 4: Perform mobile-browser acceptance checks**

Run the app on the LAN:

```bash
rtk pnpm --filter @gameboy/remote-controller dev --host 0.0.0.0
```

Use browser responsive mode plus one iOS Safari and one Android Chrome device when available. Verify the shell at `320 × 568`, `390 × 844`, `568 × 320`, and `844 × 390`: no page scroll/pull-to-refresh while dragging controls; no text selection, pinch/double-tap zoom, tap highlight, or long-press context menu on the control surface; no clipped control; safe-area padding remains visible; focus indicators still work with a keyboard; missing-token and server-unavailable copy are readable. The automated mock-server component tests are the acceptance evidence for connected input until PED-39 supplies the real server.

- [ ] **Step 5: Record the exact PED-39 handoff in review notes**

Report these implemented contracts without changing code:

```text
QR URL: http(s)://<local-host>:<port>/?token=<url-encoded-token>
WebSocket URL: ws(s)://<same-host>:<same-port>/controller
First frame: {"type":"hello","version":"v1","token":"<token>"}
Reconnect first input frame after welcome: state-sync with the complete current button set
Heartbeat: first ping 5000 ms after welcome; each later ping 5000 ms after the preceding matching pong; matching pong deadline 12000 ms after its ping
Client closes heartbeat timeout with code 4000 and reason heartbeat-timeout
Client closes after controller-disconnected with code 4001 and reason server-disconnected
Manual disconnect closes with code 1000 and reason client-disconnect
```

- [ ] **Step 6: Commit only regression fixes, if any**

If Steps 1-4 required a correction, stage only the corrected `apps/remote-controller/**` files and their regression tests:

```bash
rtk git add apps/remote-controller
rtk git commit -m "fix(remote): resolve PED-38 acceptance findings"
```

Expected: no commit is created when verification found no defect. Do not change PED-38 Linear status or publish a branch unless the task owner explicitly asks.

## Reference Notes for Executors

- Pointer capture keeps later events targeted to the pressed control until release; `pointerup` and `pointercancel` implicitly release capture, and `lostpointercapture` is an additional cleanup signal: <https://developer.mozilla.org/en-US/docs/Web/API/Pointer_events>.
- `touch-action: none` tells the browser before event dispatch that the game surface owns touch gestures and avoids browser-initiated `pointercancel`: <https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/touch-action>.
- `visibilitychange` to `hidden` is the last reliably observable lifecycle transition on mobile; `pagehide` is the fallback and remains bfcache-compatible: <https://developer.mozilla.org/en-US/docs/Web/API/Document/visibilitychange_event>, <https://developer.mozilla.org/en-US/docs/Web/API/Window/pagehide_event>.
- `overscroll-behavior: none` prevents scroll chaining and pull-to-refresh behavior on a scroll container: <https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/overscroll-behavior>.
- A WebSocket `close` event fires for client close, server close, and error-induced close, so retry logic must be idempotent across `error` plus `close`: <https://developer.mozilla.org/en-US/docs/Web/API/WebSockets_API/Writing_WebSocket_client_applications>.
- React effects that subscribe to browser lifecycle events must return cleanup functions that remove the same listeners and dispose the external session: <https://react.dev/reference/react/useEffect>.
- The manifest `display: standalone`, `start_url`, and `orientation: any` are progressive hints; unsupported browsers fall back to normal browser presentation: <https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Manifest/Reference/display>, <https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Manifest/Reference/orientation>.
