import { useCallback, useEffect, useRef, useState } from 'react'
import { normalizeRemoteError, type RemoteSessionClient, tauriRemoteSessionClient } from './remote-client'
import type { RemoteSnapshot } from './remote-types'

const offSnapshot: RemoteSnapshot = {
  phase: 'off',
  pairingUrl: null,
  expiresAtUnixMs: null,
  controllerId: null,
  latency: null,
  error: null
}

export type RemoteSessionBusy = 'starting' | 'ending' | null

export type RemoteSessionView = {
  snapshot: RemoteSnapshot
  busy: RemoteSessionBusy
  start: () => Promise<void>
  end: () => Promise<void>
}

export const useRemoteSession = (client: RemoteSessionClient = tauriRemoteSessionClient): RemoteSessionView => {
  const [snapshot, setSnapshot] = useState<RemoteSnapshot>(offSnapshot)
  const [busy, setBusy] = useState<RemoteSessionBusy>(null)
  const generationRef = useRef(0)
  const operationRef = useRef(0)
  const revisionRef = useRef(0)

  useEffect(() => {
    const generation = generationRef.current + 1
    generationRef.current = generation
    const operation = operationRef.current + 1
    operationRef.current = operation
    const revision = revisionRef.current
    let subscribed = true
    setBusy(null)
    void client
      .subscribe((nextSnapshot) => {
        if (!subscribed || generationRef.current !== generation) return
        revisionRef.current += 1
        setSnapshot(nextSnapshot)
      })
      .then((nextSnapshot) => {
        if (
          subscribed &&
          generationRef.current === generation &&
          operationRef.current === operation &&
          revisionRef.current === revision
        ) {
          revisionRef.current += 1
          setSnapshot(nextSnapshot)
        }
      })
      .catch((error) => {
        if (
          subscribed &&
          generationRef.current === generation &&
          operationRef.current === operation &&
          revisionRef.current === revision
        ) {
          revisionRef.current += 1
          setSnapshot({
            phase: 'error',
            pairingUrl: null,
            expiresAtUnixMs: null,
            controllerId: null,
            latency: null,
            error: normalizeRemoteError(error)
          })
        }
      })

    return () => {
      subscribed = false
      if (generationRef.current === generation) {
        generationRef.current += 1
        operationRef.current += 1
      }
    }
  }, [client])

  const applyAction = useCallback(
    async (nextBusy: Exclude<RemoteSessionBusy, null>, action: () => Promise<RemoteSnapshot>) => {
      const generation = generationRef.current
      const operation = operationRef.current + 1
      operationRef.current = operation
      const revision = revisionRef.current
      setBusy(nextBusy)
      try {
        const nextSnapshot = await action()
        if (
          generationRef.current === generation &&
          operationRef.current === operation &&
          revisionRef.current === revision
        ) {
          revisionRef.current += 1
          setSnapshot(nextSnapshot)
        }
      } catch (error) {
        if (
          generationRef.current === generation &&
          operationRef.current === operation &&
          revisionRef.current === revision
        ) {
          revisionRef.current += 1
          setSnapshot({
            phase: 'error',
            pairingUrl: null,
            expiresAtUnixMs: null,
            controllerId: null,
            latency: null,
            error: normalizeRemoteError(error)
          })
        }
      } finally {
        if (generationRef.current === generation && operationRef.current === operation) setBusy(null)
      }
    },
    []
  )

  const start = useCallback(() => applyAction('starting', () => client.start()), [applyAction, client])
  const end = useCallback(() => applyAction('ending', () => client.end()), [applyAction, client])

  return { snapshot, busy, start, end }
}
