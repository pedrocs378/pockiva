import { useCallback, useEffect, useRef, useState } from 'react'
import {
  type AvailableUpdate,
  tauriUpdaterClient,
  type UpdateDownloadProgress,
  type UpdaterClient
} from './updater-client'

type UpdateDetails = {
  version: string
  notes: string | null
}

export type UpdaterState =
  | { phase: 'checking' }
  | { phase: 'idle' }
  | ({ phase: 'available' } & UpdateDetails)
  | ({ phase: 'downloading'; progress: UpdateDownloadProgress } & UpdateDetails)
  | ({ phase: 'installing' } & UpdateDetails)
  | { phase: 'error'; message: string }

export type UpdaterView = {
  state: UpdaterState
  dismiss: () => Promise<void>
  install: () => Promise<void>
}

const initialProgress: UpdateDownloadProgress = {
  downloadedBytes: 0,
  totalBytes: null,
  percent: null
}

const messageFor = (error: unknown) => {
  if (error instanceof Error && error.message.trim()) return error.message
  return 'Pockiva could not install the update. Please try again later.'
}

const disposeQuietly = async (candidate: AvailableUpdate | null) => {
  if (!candidate) return
  try {
    await candidate.dispose()
  } catch {
    // Releasing an updater resource must not replace the actionable install result.
  }
}

export const useUpdater = (client: UpdaterClient = tauriUpdaterClient): UpdaterView => {
  const [state, setState] = useState<UpdaterState>({ phase: 'checking' })
  const candidateRef = useRef<AvailableUpdate | null>(null)
  const activeTokenRef = useRef<symbol | null>(null)
  const checkRef = useRef<{ client: UpdaterClient; promise: Promise<AvailableUpdate | null> } | null>(null)
  const operationRef = useRef(0)
  const installingRef = useRef(false)

  useEffect(() => {
    const token = Symbol('updater-check')
    activeTokenRef.current = token
    setState({ phase: 'checking' })

    if (checkRef.current?.client !== client) {
      checkRef.current = { client, promise: client.check() }
    }
    const checkPromise = checkRef.current.promise

    void checkPromise
      .then((candidate) => {
        if (activeTokenRef.current !== token) {
          if (activeTokenRef.current === null || checkRef.current?.client !== client) void disposeQuietly(candidate)
          return
        }
        candidateRef.current = candidate
        setState(
          candidate ? { phase: 'available', version: candidate.version, notes: candidate.notes } : { phase: 'idle' }
        )
      })
      .catch(() => {
        if (activeTokenRef.current === token) setState({ phase: 'idle' })
      })

    return () => {
      if (activeTokenRef.current === token) activeTokenRef.current = null
      if (!installingRef.current) {
        const candidate = candidateRef.current
        candidateRef.current = null
        void disposeQuietly(candidate)
      }
    }
  }, [client])

  const dismiss = useCallback(async () => {
    operationRef.current += 1
    const candidate = candidateRef.current
    candidateRef.current = null
    await disposeQuietly(candidate)
    setState({ phase: 'idle' })
  }, [])

  const install = useCallback(async () => {
    const candidate = candidateRef.current
    if (!candidate || installingRef.current) return

    const operation = operationRef.current + 1
    operationRef.current = operation
    installingRef.current = true
    const details = { version: candidate.version, notes: candidate.notes }
    setState({ phase: 'downloading', ...details, progress: initialProgress })

    try {
      await candidate.install((progress) => {
        if (operationRef.current === operation) {
          setState({ phase: 'downloading', ...details, progress })
        }
      })
      if (operationRef.current !== operation) return

      setState({ phase: 'installing', ...details })
      candidateRef.current = null
      await disposeQuietly(candidate)
      if (operationRef.current === operation) await client.relaunch()
    } catch (error) {
      if (operationRef.current === operation) {
        candidateRef.current = null
        await disposeQuietly(candidate)
        setState({ phase: 'error', message: messageFor(error) })
      }
    } finally {
      if (operationRef.current === operation) installingRef.current = false
    }
  }, [client])

  return { state, dismiss, install }
}
