import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { AvailableUpdate, UpdaterClient } from './updater-client'
import { useUpdater } from './use-updater'

const createCandidate = (): AvailableUpdate => ({
  version: '0.2.0',
  notes: 'New emulation improvements.',
  install: vi.fn().mockResolvedValue(undefined),
  dispose: vi.fn().mockResolvedValue(undefined)
})

const createClient = (candidate: AvailableUpdate | null): UpdaterClient => ({
  check: vi.fn().mockResolvedValue(candidate),
  relaunch: vi.fn().mockResolvedValue(undefined)
})

describe('useUpdater', () => {
  beforeEach(() => vi.clearAllMocks())

  it('checks once and stays silent when no update exists', async () => {
    const client = createClient(null)
    const { result, rerender } = renderHook(() => useUpdater(client))

    await waitFor(() => expect(result.current.state.phase).toBe('idle'))
    rerender()

    expect(client.check).toHaveBeenCalledOnce()
  })

  it('treats an unavailable updater endpoint as a non-blocking startup condition', async () => {
    const client = createClient(null)
    vi.mocked(client.check).mockRejectedValueOnce(new Error('latest.json returned 404'))
    const { result } = renderHook(() => useUpdater(client))

    await waitFor(() => expect(result.current.state.phase).toBe('idle'))
  })

  it('offers an available update and disposes it when postponed', async () => {
    const candidate = createCandidate()
    const client = createClient(candidate)
    const { result } = renderHook(() => useUpdater(client))

    await waitFor(() => expect(result.current.state.phase).toBe('available'))
    expect(result.current.state).toMatchObject({ version: '0.2.0', notes: 'New emulation improvements.' })

    await act(() => result.current.dismiss())

    expect(candidate.dispose).toHaveBeenCalledOnce()
    expect(result.current.state.phase).toBe('idle')
  })

  it('reports progress, installs, releases the update resource, and relaunches', async () => {
    const candidate = createCandidate()
    vi.mocked(candidate.install).mockImplementationOnce(async (onProgress) => {
      onProgress({ downloadedBytes: 25, totalBytes: 100, percent: 25 })
      onProgress({ downloadedBytes: 100, totalBytes: 100, percent: 100 })
    })
    const client = createClient(candidate)
    const { result } = renderHook(() => useUpdater(client))
    await waitFor(() => expect(result.current.state.phase).toBe('available'))

    await act(() => result.current.install())

    expect(candidate.install).toHaveBeenCalledOnce()
    expect(candidate.dispose).toHaveBeenCalledOnce()
    expect(client.relaunch).toHaveBeenCalledOnce()
    expect(result.current.state.phase).toBe('installing')
  })

  it('shows an actionable error when installation fails', async () => {
    const candidate = createCandidate()
    vi.mocked(candidate.install).mockRejectedValueOnce(new Error('signature rejected'))
    const client = createClient(candidate)
    const { result } = renderHook(() => useUpdater(client))
    await waitFor(() => expect(result.current.state.phase).toBe('available'))

    await act(() => result.current.install())

    expect(result.current.state).toEqual({
      phase: 'error',
      message: 'signature rejected'
    })
    expect(candidate.dispose).toHaveBeenCalledOnce()
    expect(client.relaunch).not.toHaveBeenCalled()
  })
})
