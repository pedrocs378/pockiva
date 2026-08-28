import { useCallback, useEffect, useRef, useState } from 'react'
import { type EmulatorRuntimeClient, tauriEmulatorRuntimeClient } from './runtime-client'
import {
  type FramePacket,
  parseRuntimeError,
  type RuntimeButton,
  type RuntimeError,
  type RuntimeSnapshot
} from './runtime-types'

const emptySnapshot: RuntimeSnapshot = { phase: 'empty', rom: null, error: null }

const normalizeError = (value: unknown): RuntimeError => {
  try {
    return parseRuntimeError(value)
  } catch {
    return {
      code: 'runtime-unavailable',
      message: value instanceof Error ? value.message : 'The desktop runtime is unavailable.'
    }
  }
}

export type EmulatorRuntimeView = {
  snapshot: RuntimeSnapshot
  openRom: () => Promise<void>
  start: () => Promise<void>
  pause: () => Promise<void>
  restart: () => Promise<void>
  close: () => Promise<void>
  setKeyboardInput: (buttons: RuntimeButton[]) => Promise<void>
  acknowledgeFrame: (sequence: number) => Promise<void>
  subscribeFrames: (consumer: (frame: FramePacket) => void) => () => void
}

export const useEmulatorRuntime = (client: EmulatorRuntimeClient = tauriEmulatorRuntimeClient): EmulatorRuntimeView => {
  const [snapshot, setSnapshot] = useState<RuntimeSnapshot>(emptySnapshot)
  const frameConsumerRef = useRef<((frame: FramePacket) => void) | null>(null)

  useEffect(() => {
    let active = true
    void client
      .subscribe({
        onSnapshot: (nextSnapshot) => {
          if (active) setSnapshot(nextSnapshot)
        },
        onFrame: (frame) => {
          if (active) frameConsumerRef.current?.(frame)
        }
      })
      .then((nextSnapshot) => {
        if (active) setSnapshot(nextSnapshot)
      })
      .catch((error) => {
        if (active) setSnapshot({ phase: 'error', rom: null, error: normalizeError(error) })
      })

    return () => {
      active = false
      frameConsumerRef.current = null
    }
  }, [client])

  const applyAction = useCallback(async (action: () => Promise<RuntimeSnapshot>) => {
    try {
      setSnapshot(await action())
    } catch (error) {
      setSnapshot({ phase: 'error', rom: null, error: normalizeError(error) })
    }
  }, [])

  const openRom = useCallback(async () => {
    try {
      const path = await client.pickRom()
      if (path === null) return
      setSnapshot({ phase: 'loading', rom: null, error: null })
      await applyAction(() => client.openRom(path))
    } catch (error) {
      setSnapshot({ phase: 'error', rom: null, error: normalizeError(error) })
    }
  }, [applyAction, client])

  const subscribeFrames = useCallback((consumer: (frame: FramePacket) => void) => {
    frameConsumerRef.current = consumer
    return () => {
      if (frameConsumerRef.current === consumer) frameConsumerRef.current = null
    }
  }, [])

  const start = useCallback(() => applyAction(() => client.start()), [applyAction, client])
  const pause = useCallback(() => applyAction(() => client.pause()), [applyAction, client])
  const restart = useCallback(() => applyAction(() => client.restart()), [applyAction, client])
  const close = useCallback(() => applyAction(() => client.close()), [applyAction, client])
  const setKeyboardInput = useCallback((buttons: RuntimeButton[]) => client.setKeyboardInput(buttons), [client])
  const acknowledgeFrame = useCallback((sequence: number) => client.acknowledgeFrame(sequence), [client])

  return {
    snapshot,
    openRom,
    start,
    pause,
    restart,
    close,
    setKeyboardInput,
    acknowledgeFrame,
    subscribeFrames
  }
}
