import { type PointerEvent as ReactPointerEvent, useEffect, useRef, useState } from 'react'
import type { Button } from '@gameboy/protocol'
import { type JoystickVector, resolveJoystickVector } from './joystick-direction'

export type VirtualJoystickProps = {
  disabled: boolean
  setPointerButtons: (pointerId: number, buttons: readonly Button[]) => void
  releasePointer: (pointerId: number) => void
}

const centered: JoystickVector = { x: 0, y: 0 }

export const VirtualJoystick = ({ disabled, setPointerButtons, releasePointer }: VirtualJoystickProps) => {
  const activePointer = useRef<number | null>(null)
  const releasePointerRef = useRef(releasePointer)
  const [knob, setKnob] = useState(centered)
  const [directions, setDirections] = useState<readonly Button[]>([])

  const update = (event: ReactPointerEvent<HTMLFieldSetElement>) => {
    if (event.pointerId !== activePointer.current) return

    const bounds = event.currentTarget.getBoundingClientRect()
    const maxDistance = Math.min(bounds.width, bounds.height) * 0.27
    const resolution = resolveJoystickVector(
      {
        x: event.clientX - (bounds.left + bounds.width / 2),
        y: event.clientY - (bounds.top + bounds.height / 2)
      },
      maxDistance
    )
    setKnob(resolution.knob)
    setDirections(resolution.buttons)
    setPointerButtons(event.pointerId, resolution.buttons)
  }

  const release = (event: ReactPointerEvent<HTMLFieldSetElement>) => {
    if (event.pointerId !== activePointer.current) return

    activePointer.current = null
    setKnob(centered)
    setDirections([])
    releasePointer(event.pointerId)
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  useEffect(() => {
    releasePointerRef.current = releasePointer
  }, [releasePointer])

  useEffect(() => {
    if (!disabled || activePointer.current === null) return

    releasePointer(activePointer.current)
    activePointer.current = null
    setKnob(centered)
    setDirections([])
  }, [disabled, releasePointer])

  useEffect(
    () => () => {
      const pointerId = activePointer.current
      if (pointerId !== null) releasePointerRef.current(pointerId)
      activePointer.current = null
    },
    []
  )

  return (
    <fieldset
      className="virtual-joystick"
      aria-label="Virtual joystick"
      aria-disabled={disabled}
      data-directions={directions.join(' ')}
      onPointerDown={(event) => {
        if (disabled || activePointer.current !== null || (event.pointerType === 'mouse' && event.button !== 0)) return
        event.preventDefault()
        activePointer.current = event.pointerId
        event.currentTarget.setPointerCapture(event.pointerId)
        update(event)
      }}
      onPointerMove={update}
      onPointerUp={release}
      onPointerCancel={release}
      onLostPointerCapture={release}
      onContextMenu={(event) => event.preventDefault()}
    >
      <div aria-hidden="true" className="joystick-knob" style={{ transform: `translate(${knob.x}px, ${knob.y}px)` }} />
      <span className="sr-only">{directions.length === 0 ? 'Centered' : directions.join(' ')}</span>
    </fieldset>
  )
}
