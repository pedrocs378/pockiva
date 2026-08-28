import { beforeEach, describe, expect, it, vi } from 'vitest'

const tauriMocks = vi.hoisted(() => ({
  check: vi.fn(),
  relaunch: vi.fn()
}))

vi.mock('@tauri-apps/plugin-updater', () => ({ check: tauriMocks.check }))
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: tauriMocks.relaunch }))

import { tauriUpdaterClient } from './updater-client'

describe('tauriUpdaterClient', () => {
  beforeEach(() => vi.clearAllMocks())

  it('returns no candidate when GitHub has no compatible release', async () => {
    tauriMocks.check.mockResolvedValueOnce(null)

    await expect(tauriUpdaterClient.check()).resolves.toBeNull()
  })

  it('maps metadata, progress, disposal, and relaunch to the Tauri plugins', async () => {
    const close = vi.fn().mockResolvedValue(undefined)
    const downloadAndInstall = vi.fn(async (onEvent: (event: unknown) => void) => {
      onEvent({ event: 'Started', data: { contentLength: 10 } })
      onEvent({ event: 'Progress', data: { chunkLength: 4 } })
      onEvent({ event: 'Progress', data: { chunkLength: 6 } })
      onEvent({ event: 'Finished' })
    })
    tauriMocks.check.mockResolvedValueOnce({
      version: '0.2.0',
      body: 'Release notes',
      close,
      downloadAndInstall
    })
    tauriMocks.relaunch.mockResolvedValueOnce(undefined)

    const candidate = await tauriUpdaterClient.check()
    const progress = vi.fn()
    await candidate?.install(progress)
    await candidate?.dispose()
    await tauriUpdaterClient.relaunch()

    expect(candidate).toMatchObject({ version: '0.2.0', notes: 'Release notes' })
    expect(progress).toHaveBeenLastCalledWith({ downloadedBytes: 10, totalBytes: 10, percent: 100 })
    expect(downloadAndInstall).toHaveBeenCalledOnce()
    expect(close).toHaveBeenCalledOnce()
    expect(tauriMocks.relaunch).toHaveBeenCalledOnce()
  })
})
