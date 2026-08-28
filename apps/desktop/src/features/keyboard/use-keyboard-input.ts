import { useEffect, useRef } from 'react'
import { type RuntimeButton, runtimeButtons } from '@/features/emulator/runtime-types'
import type { KeyboardMapping } from './keyboard-mapping'

type UseKeyboardInputOptions = {
  mapping: KeyboardMapping
  enabled: boolean
  suspended: boolean
  setKeyboardInput: (buttons: RuntimeButton[]) => Promise<void>
}

const isEditableTarget = (target: EventTarget | null): boolean => {
  if (!(target instanceof HTMLElement)) return false
  return (
    target.matches('input, textarea, select, [contenteditable="true"]') ||
    target.closest('input, textarea, select, [contenteditable="true"]') !== null
  )
}

export const useKeyboardInput = ({ mapping, enabled, suspended, setKeyboardInput }: UseKeyboardInputOptions): void => {
  const pressedRef = useRef(new Set<RuntimeButton>())
  const writeChainRef = useRef<Promise<void>>(Promise.resolve())

  useEffect(() => {
    const enqueueSnapshot = () => {
      const snapshot = runtimeButtons.filter((button) => pressedRef.current.has(button))
      writeChainRef.current = writeChainRef.current
        .catch(() => undefined)
        .then(() => setKeyboardInput(snapshot))
        .catch(() => undefined)
    }

    const releaseAll = () => {
      if (pressedRef.current.size === 0) return
      pressedRef.current.clear()
      enqueueSnapshot()
    }

    const buttonForCode = (code: string) => runtimeButtons.find((button) => mapping[button] === code)

    const onKeyDown = (event: KeyboardEvent) => {
      if (!enabled || suspended || event.repeat || isEditableTarget(event.target)) return
      const button = buttonForCode(event.code)
      if (!button) return
      event.preventDefault()
      if (pressedRef.current.has(button)) return
      pressedRef.current.add(button)
      enqueueSnapshot()
    }

    const onKeyUp = (event: KeyboardEvent) => {
      const button = buttonForCode(event.code)
      if (!button || !pressedRef.current.delete(button)) return
      enqueueSnapshot()
    }

    const onVisibilityChange = () => {
      if (document.visibilityState === 'hidden') releaseAll()
    }

    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('keyup', onKeyUp)
    window.addEventListener('blur', releaseAll)
    document.addEventListener('visibilitychange', onVisibilityChange)

    if (!enabled || suspended) releaseAll()

    return () => {
      window.removeEventListener('keydown', onKeyDown)
      window.removeEventListener('keyup', onKeyUp)
      window.removeEventListener('blur', releaseAll)
      document.removeEventListener('visibilitychange', onVisibilityChange)
      releaseAll()
    }
  }, [enabled, mapping, setKeyboardInput, suspended])
}
