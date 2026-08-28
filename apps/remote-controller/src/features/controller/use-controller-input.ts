import { useCallback, useEffect, useRef, useState } from 'react'
import type { Button } from '@gameboy/protocol'
import type { ControllerSession } from '@/features/session/controller-session'
import { type ButtonTransition, PointerButtonTracker } from './pointer-button-tracker'

export type ControllerInputState = {
  pressedButtons: ReadonlySet<Button>
  pressPointer: (pointerId: number, button: Button) => void
  setPointerButtons: (pointerId: number, buttons: readonly Button[]) => void
  releasePointer: (pointerId: number) => void
  releaseButtons: (buttons: readonly Button[]) => void
  releaseAll: () => void
}

export const useControllerInput = (session: ControllerSession | null): ControllerInputState => {
  const trackerRef = useRef<PointerButtonTracker | null>(null)
  if (!trackerRef.current) trackerRef.current = new PointerButtonTracker()
  const tracker = trackerRef.current
  const [pressedButtons, setPressedButtons] = useState<ReadonlySet<Button>>(() => new Set())

  const applyTransitions = useCallback(
    (transitions: readonly ButtonTransition[]) => {
      if (transitions.length === 0) return
      setPressedButtons(new Set(tracker.pressedButtons()))
      for (const transition of transitions) session?.setButton(transition.button, transition.pressed)
    },
    [session, tracker]
  )

  const setPointerButtons = useCallback(
    (pointerId: number, buttons: readonly Button[]) => applyTransitions(tracker.set(pointerId, buttons)),
    [applyTransitions, tracker]
  )

  const pressPointer = useCallback(
    (pointerId: number, button: Button) => applyTransitions(tracker.press(pointerId, button)),
    [applyTransitions, tracker]
  )

  const releasePointer = useCallback(
    (pointerId: number) => applyTransitions(tracker.release(pointerId)),
    [applyTransitions, tracker]
  )

  const releaseButtons = useCallback(
    (buttons: readonly Button[]) => applyTransitions(tracker.releaseButtons(buttons)),
    [applyTransitions, tracker]
  )

  const releaseAll = useCallback(() => {
    tracker.clear()
    setPressedButtons(new Set())
    session?.syncButtons([])
  }, [session, tracker])

  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'hidden') releaseAll()
    }
    const handlePageHide = () => releaseAll()

    document.addEventListener('visibilitychange', handleVisibilityChange)
    window.addEventListener('pagehide', handlePageHide)
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange)
      window.removeEventListener('pagehide', handlePageHide)
      releaseAll()
    }
  }, [releaseAll])

  return { pressedButtons, pressPointer, setPointerButtons, releasePointer, releaseButtons, releaseAll }
}
