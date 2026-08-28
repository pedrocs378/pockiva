import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { RemoteSessionClient } from './remote-client'
import type { RemoteSnapshot } from './remote-types'
import { useRemoteSession } from './use-remote-session'

const offSnapshot: RemoteSnapshot = {
  phase: 'off',
  pairingUrl: null,
  expiresAtUnixMs: null,
  controllerId: null,
  latency: null,
  error: null
}

const waitingSnapshot: RemoteSnapshot = {
  phase: 'waiting',
  pairingUrl: 'http://192.168.1.10:4173/?token=secret',
  expiresAtUnixMs: 1_800_000_000_000,
  controllerId: null,
  latency: null,
  error: null
}

const connectedSnapshot: RemoteSnapshot = {
  ...waitingSnapshot,
  phase: 'connected',
  controllerId: 'controller-1'
}

const deferred = <T,>() => {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise
  })
  return { promise, resolve }
}

const createClient = () => {
  let onSnapshot: ((snapshot: RemoteSnapshot) => void) | null = null
  const client: RemoteSessionClient = {
    subscribe: vi.fn(async (handler) => {
      onSnapshot = handler
      return offSnapshot
    }),
    snapshot: vi.fn().mockResolvedValue(offSnapshot),
    start: vi.fn().mockResolvedValue(waitingSnapshot),
    end: vi.fn().mockResolvedValue(offSnapshot)
  }
  return { client, emit: (snapshot: RemoteSnapshot) => onSnapshot?.(snapshot) }
}

describe('useRemoteSession', () => {
  beforeEach(() => vi.clearAllMocks())

  it('subscribes once and applies authoritative snapshots', async () => {
    const { client, emit } = createClient()
    const { result, rerender } = renderHook(() => useRemoteSession(client))

    await waitFor(() => expect(client.subscribe).toHaveBeenCalledOnce())
    act(() => emit(waitingSnapshot))
    expect(result.current.snapshot).toEqual(waitingSnapshot)

    rerender()
    expect(client.subscribe).toHaveBeenCalledOnce()
  })

  it('does not let the initial subscribe response overwrite a newer channel event', async () => {
    const { client } = createClient()
    const subscriptionPending = deferred<RemoteSnapshot>()
    let onSnapshot: ((snapshot: RemoteSnapshot) => void) | null = null
    vi.mocked(client.subscribe).mockImplementationOnce(async (handler) => {
      onSnapshot = handler
      return subscriptionPending.promise
    })
    const { result } = renderHook(() => useRemoteSession(client))
    await waitFor(() => expect(client.subscribe).toHaveBeenCalledOnce())

    act(() => onSnapshot?.(connectedSnapshot))
    await act(async () => {
      subscriptionPending.resolve(offSnapshot)
      await subscriptionPending.promise
    })

    expect(result.current.snapshot).toEqual(connectedSnapshot)
  })

  it('exposes starting and ending busy states and applies action results', async () => {
    const { client } = createClient()
    const startPending = deferred<RemoteSnapshot>()
    const endPending = deferred<RemoteSnapshot>()
    vi.mocked(client.start).mockReturnValueOnce(startPending.promise)
    vi.mocked(client.end).mockReturnValueOnce(endPending.promise)
    const { result } = renderHook(() => useRemoteSession(client))

    let startOperation!: Promise<void>
    act(() => {
      startOperation = result.current.start()
    })
    expect(result.current.busy).toBe('starting')

    await act(async () => {
      startPending.resolve(waitingSnapshot)
      await startOperation
    })
    expect(result.current.snapshot).toEqual(waitingSnapshot)
    expect(result.current.busy).toBeNull()

    let endOperation!: Promise<void>
    act(() => {
      endOperation = result.current.end()
    })
    expect(result.current.busy).toBe('ending')

    await act(async () => {
      endPending.resolve(offSnapshot)
      await endOperation
    })
    expect(result.current.snapshot).toEqual(offSnapshot)
    expect(result.current.busy).toBeNull()
  })

  it('does not let an action response overwrite a newer channel event', async () => {
    const { client, emit } = createClient()
    const startPending = deferred<RemoteSnapshot>()
    vi.mocked(client.start).mockReturnValueOnce(startPending.promise)
    const { result } = renderHook(() => useRemoteSession(client))
    await waitFor(() => expect(client.subscribe).toHaveBeenCalledOnce())

    let operation!: Promise<void>
    act(() => {
      operation = result.current.start()
    })
    act(() => emit(connectedSnapshot))
    await act(async () => {
      startPending.resolve(waitingSnapshot)
      await operation
    })

    expect(result.current.snapshot).toEqual(connectedSnapshot)
    expect(result.current.busy).toBeNull()
  })

  it('normalizes subscription and action failures into the remote error state', async () => {
    const { client } = createClient()
    vi.mocked(client.subscribe).mockRejectedValueOnce(new Error('subscription failed'))
    const { result } = renderHook(() => useRemoteSession(client))

    await waitFor(() => expect(result.current.snapshot.phase).toBe('error'))
    expect(result.current.snapshot.error).toEqual({
      code: 'runtime-unavailable',
      message: 'subscription failed'
    })

    vi.mocked(client.start).mockRejectedValueOnce({ code: 'bind-failed', message: 'Port unavailable.' })
    await act(() => result.current.start())
    expect(result.current.snapshot.error).toEqual({ code: 'bind-failed', message: 'Port unavailable.' })
  })

  it('ignores subscription and action results after unmount', async () => {
    const { client, emit } = createClient()
    const pending = deferred<RemoteSnapshot>()
    vi.mocked(client.start).mockReturnValueOnce(pending.promise)
    const { result, unmount } = renderHook(() => useRemoteSession(client))
    const operation = result.current.start()

    unmount()
    act(() => emit(waitingSnapshot))
    pending.resolve(waitingSnapshot)

    await expect(operation).resolves.toBeUndefined()
  })

  it('ignores an old client action that resolves after a client change', async () => {
    const old = createClient()
    const current = createClient()
    vi.mocked(current.client.subscribe).mockResolvedValueOnce(connectedSnapshot)
    const oldStartPending = deferred<RemoteSnapshot>()
    vi.mocked(old.client.start).mockReturnValueOnce(oldStartPending.promise)
    const { result, rerender } = renderHook(({ client }) => useRemoteSession(client), {
      initialProps: { client: old.client }
    })
    await waitFor(() => expect(old.client.subscribe).toHaveBeenCalledOnce())

    let oldOperation!: Promise<void>
    act(() => {
      oldOperation = result.current.start()
    })
    rerender({ client: current.client })
    await waitFor(() => expect(result.current.snapshot).toEqual(connectedSnapshot))

    await act(async () => {
      oldStartPending.resolve(waitingSnapshot)
      await oldOperation
    })

    expect(result.current.snapshot).toEqual(connectedSnapshot)
    expect(result.current.busy).toBeNull()
  })
})
