import { useCallback, useEffect, useRef, useState } from 'react'
import type { Button } from '@gameboy/protocol'
import type { ControllerSession } from '@/features/session/controller-session'
import { PointerButtonTracker } from './pointer-button-tracker'

export type ControllerInputState = {
  pressedButtons: ReadonlySet<Button>
  pressPointer: (pointerId: number, button: Button) => void
  releasePointer: (pointerId: number) => void
  releaseAll: () => void
}

export const useControllerInput = (session: ControllerSession | null): ControllerInputState => {
  const trackerRef = useRef<PointerButtonTracker | null>(null)
  if (!trackerRef.current) trackerRef.current = new PointerButtonTracker()
  const tracker = trackerRef.current
  const [pressedButtons, setPressedButtons] = useState<ReadonlySet<Button>>(() => new Set())

  const pressPointer = useCallback(
    (pointerId: number, button: Button) => {
      const transition = tracker.press(pointerId, button)
      if (!transition) return
      setPressedButtons(new Set(tracker.pressedButtons()))
      session?.setButton(transition.button, transition.pressed)
    },
    [session, tracker]
  )

  const releasePointer = useCallback(
    (pointerId: number) => {
      const transition = tracker.release(pointerId)
      if (!transition) return
      setPressedButtons(new Set(tracker.pressedButtons()))
      session?.setButton(transition.button, transition.pressed)
    },
    [session, tracker]
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

  return { pressedButtons, pressPointer, releasePointer, releaseAll }
}
