import { Channel, invoke } from '@tauri-apps/api/core'
import {
  parseRemoteError,
  parseRemoteEvent,
  parseRemoteSnapshot,
  type RemoteError,
  type RemoteSnapshot
} from './remote-types'

export interface RemoteSessionClient {
  subscribe(onSnapshot: (snapshot: RemoteSnapshot) => void): Promise<RemoteSnapshot>
  snapshot(): Promise<RemoteSnapshot>
  start(): Promise<RemoteSnapshot>
  end(): Promise<RemoteSnapshot>
}

export const normalizeRemoteError = (value: unknown): RemoteError => {
  try {
    return parseRemoteError(value)
  } catch {
    const message =
      typeof value === 'string' ? value : value instanceof Error ? value.message : 'The remote session is unavailable.'
    return { code: 'runtime-unavailable', message }
  }
}

const call = async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    throw normalizeRemoteError(error)
  }
}

const callForSnapshot = async (command: string, args?: Record<string, unknown>) =>
  parseRemoteSnapshot(await call<unknown>(command, args))

export const tauriRemoteSessionClient: RemoteSessionClient = {
  async subscribe(onSnapshot) {
    const events = new Channel<unknown>()
    events.onmessage = (payload) => {
      const event = parseRemoteEvent(payload)
      onSnapshot(event.snapshot)
    }
    return callForSnapshot('subscribe_remote', { events })
  },

  snapshot: () => callForSnapshot('remote_snapshot'),
  start: () => callForSnapshot('start_remote_session'),
  end: () => callForSnapshot('end_remote_session')
}
