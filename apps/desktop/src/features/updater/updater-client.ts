import { relaunch } from '@tauri-apps/plugin-process'
import { check, type DownloadEvent } from '@tauri-apps/plugin-updater'

export type UpdateDownloadProgress = {
  downloadedBytes: number
  totalBytes: number | null
  percent: number | null
}

export type AvailableUpdate = {
  version: string
  notes: string | null
  install: (onProgress: (progress: UpdateDownloadProgress) => void) => Promise<void>
  dispose: () => Promise<void>
}

export type UpdaterClient = {
  check: () => Promise<AvailableUpdate | null>
  relaunch: () => Promise<void>
}

const percentage = (downloadedBytes: number, totalBytes: number | null) => {
  if (!totalBytes || totalBytes <= 0) return null
  return Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
}

export const tauriUpdaterClient: UpdaterClient = {
  async check() {
    const update = await check()
    if (!update) return null

    return {
      version: update.version,
      notes: update.body?.trim() || null,
      install: async (onProgress) => {
        let downloadedBytes = 0
        let totalBytes: number | null = null

        const report = (event: DownloadEvent) => {
          if (event.event === 'Started') {
            downloadedBytes = 0
            totalBytes = event.data.contentLength ?? null
          } else if (event.event === 'Progress') {
            downloadedBytes += event.data.chunkLength
          } else if (totalBytes !== null) {
            downloadedBytes = totalBytes
          }

          onProgress({
            downloadedBytes,
            totalBytes,
            percent: percentage(downloadedBytes, totalBytes)
          })
        }

        await update.downloadAndInstall(report)
      },
      dispose: () => update.close()
    }
  },
  relaunch
}
