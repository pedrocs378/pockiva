import type { PointerEvent as ReactPointerEvent } from 'react'
import type { Button } from '@gameboy/protocol'
import { Button as UiButton } from '@/components/ui/button'

export type ControllerButtonProps = {
  button: Button
  label: string
  className?: string
  pressed: boolean
  disabled: boolean
  onPress: (pointerId: number, button: Button) => void
  onRelease: (pointerId: number) => void
}

export const ControllerButton = ({
  button,
  label,
  className,
  pressed,
  disabled,
  onPress,
  onRelease
}: ControllerButtonProps) => {
  const handlePointerDown = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (disabled || (event.pointerType === 'mouse' && event.button !== 0)) return
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    onPress(event.pointerId, button)
  }

  const handlePointerEnd = (event: ReactPointerEvent<HTMLButtonElement>) => {
    event.preventDefault()
    onRelease(event.pointerId)
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
  }

  return (
    <UiButton
      type="button"
      variant="unstyled"
      size="auto"
      className={className}
      disabled={disabled}
      aria-pressed={pressed}
      data-button={button}
      data-pressed={pressed}
      onPointerDown={handlePointerDown}
      onPointerUp={handlePointerEnd}
      onPointerCancel={handlePointerEnd}
      onLostPointerCapture={(event) => onRelease(event.pointerId)}
      onContextMenu={(event) => event.preventDefault()}
    >
      {label}
    </UiButton>
  )
}
