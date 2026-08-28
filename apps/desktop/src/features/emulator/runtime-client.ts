import { Channel, invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import {
  decodeFramePacket,
  type FramePacket,
  parseRuntimeError,
  parseRuntimeEvent,
  parseRuntimeSnapshot,
  type RuntimeButton,
  type RuntimeError,
  type RuntimeSnapshot
} from './runtime-types'

export type RuntimeSubscription = {
  onSnapshot: (snapshot: RuntimeSnapshot) => void
  onFrame: (frame: FramePacket) => void
}

export interface EmulatorRuntimeClient {
  pickRom(): Promise<string | null>
  subscribe(handlers: RuntimeSubscription): Promise<RuntimeSnapshot>
  snapshot(): Promise<RuntimeSnapshot>
  openRom(path: string): Promise<RuntimeSnapshot>
  start(): Promise<RuntimeSnapshot>
  pause(): Promise<RuntimeSnapshot>
  restart(): Promise<RuntimeSnapshot>
  close(): Promise<RuntimeSnapshot>
  setKeyboardInput(buttons: RuntimeButton[]): Promise<void>
  acknowledgeFrame(sequence: number): Promise<void>
}

const normalizeRuntimeError = (value: unknown): RuntimeError => {
  const parsed = parseRuntimeErrorSafe(value)
  if (parsed) {
    return parsed
  }

  const message =
    typeof value === 'string' ? value : value instanceof Error ? value.message : 'The desktop runtime is unavailable.'
  return { code: 'runtime-unavailable', message }
}

const parseRuntimeErrorSafe = (value: unknown): RuntimeError | null => {
  try {
    return parseRuntimeError(value)
  } catch {
    return null
  }
}

const call = async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    throw normalizeRuntimeError(error)
  }
}

const callForSnapshot = async (command: string, args?: Record<string, unknown>) =>
  parseRuntimeSnapshot(await call<unknown>(command, args))

export const tauriEmulatorRuntimeClient: EmulatorRuntimeClient = {
  async pickRom() {
    try {
      const path = await open({
        directory: false,
        multiple: false,
        title: 'Open Game Boy ROM',
        filters: [{ name: 'Game Boy ROM', extensions: ['gb', 'gbc'] }]
      })
      if (path === null || typeof path === 'string') {
        return path
      }
      throw new TypeError('ROM picker returned multiple paths')
    } catch (error) {
      throw normalizeRuntimeError(error)
    }
  },

  async subscribe(handlers) {
    const events = new Channel<unknown>()
    const frames = new Channel<ArrayBuffer>()
    events.onmessage = (payload) => {
      const event = parseRuntimeEvent(payload)
      handlers.onSnapshot(event.snapshot)
    }
    frames.onmessage = (payload) => handlers.onFrame(decodeFramePacket(payload))
    return callForSnapshot('subscribe_runtime', { events, frames })
  },

  snapshot: () => callForSnapshot('runtime_snapshot'),
  openRom: (path) => callForSnapshot('open_rom', { path }),
  start: () => callForSnapshot('start_rom'),
  pause: () => callForSnapshot('pause_rom'),
  restart: () => callForSnapshot('restart_rom'),
  close: () => callForSnapshot('close_rom'),
  setKeyboardInput: (buttons) => call<void>('set_keyboard_input', { buttons }),
  acknowledgeFrame: (sequence) => call<void>('acknowledge_frame', { sequence })
}
