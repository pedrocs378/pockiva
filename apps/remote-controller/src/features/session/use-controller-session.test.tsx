import { act, fireEvent, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MockControllerServer } from '@/test/mock-controller-server'
import { useControllerSession } from './use-controller-session'

const pairing = {
  status: 'ready',
  config: { token: 'pairing-token', socketUrl: 'ws://gb.local/controller' }
} as const

afterEach(() => {
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' })
  vi.clearAllTimers()
  vi.useRealTimers()
})

describe('useControllerSession activity', () => {
  it('does not reconnect after a hidden-page drop until visibility returns', async () => {
    vi.useFakeTimers()
    const server = new MockControllerServer()
    const transport = server.createTransport()
    const { result } = renderHook(() => useControllerSession(pairing, transport))
    await act(async () => await Promise.resolve())
    expect(result.current.snapshot.status).toBe('connected')

    Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' })
    fireEvent(document, new Event('visibilitychange'))
    act(() => server.dropConnection())
    await act(async () => await vi.advanceTimersByTimeAsync(60_000))
    expect(server.connectionCount).toBe(1)

    Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' })
    fireEvent(window, new Event('pageshow'))
    await act(async () => await Promise.resolve())
    expect(server.connectionCount).toBe(2)
    expect(result.current.snapshot.status).toBe('connected')
  })

  it('pauses recovery on pagehide and resumes on the next visible signal', async () => {
    vi.useFakeTimers()
    const server = new MockControllerServer()
    const transport = server.createTransport()
    renderHook(() => useControllerSession(pairing, transport))
    await act(async () => await Promise.resolve())

    fireEvent(window, new Event('pagehide'))
    act(() => server.dropConnection())
    await act(async () => await vi.advanceTimersByTimeAsync(60_000))
    expect(server.connectionCount).toBe(1)

    fireEvent(document, new Event('visibilitychange'))
    await act(async () => await Promise.resolve())
    expect(server.connectionCount).toBe(2)
  })
})
