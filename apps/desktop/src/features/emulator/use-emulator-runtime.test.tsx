import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { EmulatorRuntimeClient, RuntimeSubscription } from './runtime-client'
import type { FramePacket, RuntimeSnapshot } from './runtime-types'
import { useEmulatorRuntime } from './use-emulator-runtime'

const emptySnapshot: RuntimeSnapshot = { phase: 'empty', rom: null, error: null }
const pausedSnapshot: RuntimeSnapshot = {
  phase: 'paused',
  rom: {
    title: 'Fixture',
    fileName: 'fixture.gb',
    romIdentity: 'fixture',
    mapper: 'rom-only',
    compatibility: 'dmg'
  },
  error: null
}
const runningSnapshot: RuntimeSnapshot = { ...pausedSnapshot, phase: 'running' }

const deferred = <T,>() => {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise
  })
  return { promise, resolve }
}

const createClient = () => {
  let subscription: RuntimeSubscription | null = null
  const client: EmulatorRuntimeClient = {
    pickRom: vi.fn().mockResolvedValue('/private/fixture.gb'),
    subscribe: vi.fn(async (handlers) => {
      subscription = handlers
      return emptySnapshot
    }),
    snapshot: vi.fn().mockResolvedValue(emptySnapshot),
    openRom: vi.fn().mockResolvedValue(pausedSnapshot),
    start: vi.fn().mockResolvedValue(runningSnapshot),
    pause: vi.fn().mockResolvedValue(pausedSnapshot),
    restart: vi.fn().mockResolvedValue(runningSnapshot),
    close: vi.fn().mockResolvedValue(emptySnapshot),
    setKeyboardInput: vi.fn().mockResolvedValue(undefined),
    acknowledgeFrame: vi.fn().mockResolvedValue(undefined)
  }
  return { client, getSubscription: () => subscription }
}

describe('useEmulatorRuntime', () => {
  beforeEach(() => vi.clearAllMocks())

  it('starts empty and applies authoritative channel snapshots', async () => {
    const { client, getSubscription } = createClient()
    const { result } = renderHook(() => useEmulatorRuntime(client))

    expect(result.current.snapshot).toEqual(emptySnapshot)
    await waitFor(() => expect(getSubscription()).not.toBeNull())

    act(() => getSubscription()?.onSnapshot(pausedSnapshot))
    expect(result.current.snapshot).toEqual(pausedSnapshot)
  })

  it('shows loading immediately while opening and applies the result', async () => {
    const { client } = createClient()
    const pending = deferred<RuntimeSnapshot>()
    vi.mocked(client.openRom).mockReturnValueOnce(pending.promise)
    const { result } = renderHook(() => useEmulatorRuntime(client))

    let operation!: Promise<void>
    await act(async () => {
      operation = result.current.openRom()
      await Promise.resolve()
    })
    expect(result.current.snapshot.phase).toBe('loading')

    await act(async () => {
      pending.resolve(pausedSnapshot)
      await operation
    })
    expect(result.current.snapshot).toEqual(pausedSnapshot)
  })

  it('keeps the prior snapshot when ROM selection is cancelled', async () => {
    const { client } = createClient()
    vi.mocked(client.pickRom).mockResolvedValueOnce(null)
    const { result } = renderHook(() => useEmulatorRuntime(client))

    await act(() => result.current.openRom())

    expect(result.current.snapshot).toEqual(emptySnapshot)
    expect(client.openRom).not.toHaveBeenCalled()
  })

  it('preserves typed open failures in the error state', async () => {
    const { client } = createClient()
    vi.mocked(client.openRom).mockRejectedValueOnce({ code: 'invalid-rom', message: 'bad header' })
    const { result } = renderHook(() => useEmulatorRuntime(client))

    await act(() => result.current.openRom())

    expect(result.current.snapshot).toEqual({
      phase: 'error',
      rom: null,
      error: { code: 'invalid-rom', message: 'bad header' }
    })
  })

  it('delivers frames outside React state', async () => {
    const { client, getSubscription } = createClient()
    const { result } = renderHook(() => useEmulatorRuntime(client))
    await waitFor(() => expect(getSubscription()).not.toBeNull())
    const snapshotBeforeFrame = result.current.snapshot
    const consumer = vi.fn()
    result.current.subscribeFrames(consumer)
    const frame = { sequence: 1, width: 160, height: 144, rgba: new Uint8ClampedArray(92_160) } as FramePacket

    act(() => getSubscription()?.onFrame(frame))

    expect(consumer).toHaveBeenCalledWith(frame)
    expect(result.current.snapshot).toBe(snapshotBeforeFrame)
  })

  it('maps lifecycle actions only to their corresponding client method', async () => {
    const { client } = createClient()
    const { result } = renderHook(() => useEmulatorRuntime(client))

    await act(() => result.current.start())
    await act(() => result.current.pause())
    await act(() => result.current.restart())
    await act(() => result.current.close())

    expect(client.start).toHaveBeenCalledOnce()
    expect(client.pause).toHaveBeenCalledOnce()
    expect(client.restart).toHaveBeenCalledOnce()
    expect(client.close).toHaveBeenCalledOnce()
  })
})
