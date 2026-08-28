import { beforeEach, describe, expect, it, vi } from 'vitest'

type MockChannel<T> = { onmessage: (message: T) => void }

const tauriMocks = vi.hoisted(() => {
  class Channel<T> {
    onmessage: (message: T) => void = () => undefined
  }

  return {
    Channel,
    channels: [] as Array<MockChannel<unknown>>,
    invoke: vi.fn()
  }
})

vi.mock('@tauri-apps/api/core', () => ({
  Channel: class<T> extends tauriMocks.Channel<T> {
    constructor() {
      super()
      tauriMocks.channels.push(this as MockChannel<unknown>)
    }
  },
  invoke: tauriMocks.invoke
}))

import { tauriRemoteSessionClient as client } from './remote-client'

const offSnapshot = {
  phase: 'off',
  pairingUrl: null,
  expiresAtUnixMs: null,
  controllerId: null,
  latency: null,
  error: null
}

const waitingSnapshot = {
  phase: 'waiting',
  pairingUrl: 'http://192.168.1.10:4173/?token=secret',
  expiresAtUnixMs: 1_800_000_000_000,
  controllerId: null,
  latency: null,
  error: null
}

describe('tauri remote session client', () => {
  beforeEach(() => {
    tauriMocks.invoke.mockReset()
    tauriMocks.channels.length = 0
  })

  it('maps lifecycle methods to exact Tauri commands without arguments', async () => {
    tauriMocks.invoke.mockResolvedValue(offSnapshot)

    await client.snapshot()
    expect(tauriMocks.invoke).toHaveBeenLastCalledWith('remote_snapshot', undefined)
    await client.start()
    expect(tauriMocks.invoke).toHaveBeenLastCalledWith('start_remote_session', undefined)
    await client.end()
    expect(tauriMocks.invoke).toHaveBeenLastCalledWith('end_remote_session', undefined)
  })

  it('parses the immediate snapshot and every pushed channel event', async () => {
    tauriMocks.invoke.mockResolvedValue(waitingSnapshot)
    const onSnapshot = vi.fn()

    await expect(client.subscribe(onSnapshot)).resolves.toEqual(waitingSnapshot)

    expect(tauriMocks.channels).toHaveLength(1)
    expect(tauriMocks.invoke).toHaveBeenCalledWith('subscribe_remote', { events: tauriMocks.channels[0] })
    tauriMocks.channels[0]?.onmessage({ type: 'snapshot', snapshot: offSnapshot })
    expect(onSnapshot).toHaveBeenCalledWith(offSnapshot)
    expect(() => tauriMocks.channels[0]?.onmessage({ type: 'snapshot', snapshot: { phase: 'unknown' } })).toThrow()
  })

  it('rejects malformed command snapshots at the native boundary', async () => {
    tauriMocks.invoke.mockResolvedValue({ ...offSnapshot, pairingUrl: 'http://token-leak.test' })

    await expect(client.snapshot()).rejects.toThrow()
  })

  it('preserves native typed errors and normalizes unknown failures', async () => {
    tauriMocks.invoke.mockRejectedValueOnce({ code: 'no-lan-address', message: 'No local network.' })
    await expect(client.start()).rejects.toEqual({ code: 'no-lan-address', message: 'No local network.' })

    tauriMocks.invoke.mockRejectedValueOnce('native bridge failed')
    await expect(client.snapshot()).rejects.toEqual({
      code: 'runtime-unavailable',
      message: 'native bridge failed'
    })
  })
})
