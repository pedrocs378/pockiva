import { afterEach, describe, expect, it, vi } from 'vitest'
import { MockControllerServer } from '@/test/mock-controller-server'
import { ControllerSession, type ControllerSessionOptions } from './controller-session'

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

describe('ControllerSession', () => {
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

  it.each(['{', JSON.stringify({ type: 'welcome', version: 'v2', controllerId: 'controller-1' })])(
    'treats malformed server payload %s as incompatible protocol',
    async (payload) => {
      const server = new MockControllerServer()
      const session = createSession(server)
      session.connect()
      await flushMicrotasks()
      server.sendRaw(payload)

      expect(session.getSnapshot().status).toBe('incompatible-protocol')
    }
  )

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
      type: 'state-sync',
      buttons: ['left', 'b'],
      sequence: 2
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
    await vi.runOnlyPendingTimersAsync()

    expect({
      connectionCount: server.connectionCount,
      status: session.getSnapshot().status,
      reconnectAttempt: session.getSnapshot().reconnectAttempt
    }).toEqual({ connectionCount: 2, status: 'connected', reconnectAttempt: 0 })
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
    expect(server.receivedMessages.filter(({ type }) => type === 'ping')).toEqual([{ type: 'ping', sequence: 1 }])
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
})
