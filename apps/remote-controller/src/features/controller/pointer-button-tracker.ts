import type { Button } from '@gameboy/protocol'
import { BUTTON_ORDER } from '@/constants/controller'

export type ButtonTransition = { button: Button; pressed: boolean }

export class PointerButtonTracker {
  private readonly pointers = new Map<number, Button>()

  press(pointerId: number, button: Button): ButtonTransition | null {
    if (this.pointers.has(pointerId)) return null
    const wasPressed = [...this.pointers.values()].includes(button)
    this.pointers.set(pointerId, button)
    return wasPressed ? null : { button, pressed: true }
  }

  release(pointerId: number): ButtonTransition | null {
    const button = this.pointers.get(pointerId)
    if (!button) return null
    this.pointers.delete(pointerId)
    return [...this.pointers.values()].includes(button) ? null : { button, pressed: false }
  }

  clear(): Button[] {
    const buttons = this.pressedButtons()
    this.pointers.clear()
    return buttons
  }

  pressedButtons(): Button[] {
    const pressed = new Set(this.pointers.values())
    return BUTTON_ORDER.filter((button) => pressed.has(button))
  }
}
