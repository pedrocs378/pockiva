import { useCallback, useEffect, useMemo, useSyncExternalStore } from 'react'
import type { Button } from '@gameboy/protocol'
import { ControllerSession, type SessionSnapshot } from './controller-session'
import type { PairingResult } from './pairing'
import type { SessionTransport } from './transport'

type MissingTokenSnapshot = {
  status: 'missing-token'
  controllerId: null
  pressedButtons: ReadonlySet<Button>
  reconnectAttempt: 0
}

export type UseControllerSessionResult = {
  session: ControllerSession | null
  snapshot: SessionSnapshot | MissingTokenSnapshot
  connect: () => void
  disconnect: () => void
}

const MISSING_TOKEN_SNAPSHOT: MissingTokenSnapshot = {
  status: 'missing-token',
  controllerId: null,
  pressedButtons: new Set(),
  reconnectAttempt: 0
}

const subscribeMissing = () => () => undefined
const getMissingSnapshot = () => MISSING_TOKEN_SNAPSHOT

export const useControllerSession = (
  pairing: PairingResult,
  transport: SessionTransport
): UseControllerSessionResult => {
  const session = useMemo(
    () => (pairing.status === 'ready' ? new ControllerSession({ pairing: pairing.config, transport }) : null),
    [pairing, transport]
  )
  const snapshot = useSyncExternalStore(
    session?.subscribe ?? subscribeMissing,
    session?.getSnapshot ?? getMissingSnapshot,
    session?.getSnapshot ?? getMissingSnapshot
  )

  useEffect(() => {
    session?.connect()
    return () => session?.disconnect()
  }, [session])

  const connect = useCallback(() => session?.connect(), [session])
  const disconnect = useCallback(() => session?.disconnect(), [session])

  return { session, snapshot, connect, disconnect }
}
