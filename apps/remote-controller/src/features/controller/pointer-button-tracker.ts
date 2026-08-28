import type { Button } from '@gameboy/protocol'
import { BUTTON_ORDER } from '@/constants/controller'

export type ButtonTransition = { button: Button; pressed: boolean }

export class PointerButtonTracker {
  private readonly pointers = new Map<number, ReadonlySet<Button>>()

  set(pointerId: number, buttons: readonly Button[]): ButtonTransition[] {
    const before = new Set(this.pressedButtons())
    const next = new Set(buttons)
    if (next.size === 0) this.pointers.delete(pointerId)
    else this.pointers.set(pointerId, next)
    return this.transitions(before, new Set(this.pressedButtons()))
  }

  press(pointerId: number, button: Button): ButtonTransition[] {
    return this.set(pointerId, [button])
  }

  release(pointerId: number): ButtonTransition[] {
    return this.set(pointerId, [])
  }

  releaseButtons(buttons: readonly Button[]): ButtonTransition[] {
    const removed = new Set(buttons)
    const before = new Set(this.pressedButtons())
    for (const [pointerId, owned] of this.pointers) {
      const retained = [...owned].filter((button) => !removed.has(button))
      if (retained.length === 0) this.pointers.delete(pointerId)
      else this.pointers.set(pointerId, new Set(retained))
    }
    return this.transitions(before, new Set(this.pressedButtons()))
  }

  clear(): Button[] {
    const buttons = this.pressedButtons()
    this.pointers.clear()
    return buttons
  }

  pressedButtons(): Button[] {
    const pressed = new Set([...this.pointers.values()].flatMap((buttons) => [...buttons]))
    return BUTTON_ORDER.filter((button) => pressed.has(button))
  }

  private transitions(before: ReadonlySet<Button>, after: ReadonlySet<Button>): ButtonTransition[] {
    return BUTTON_ORDER.flatMap((button) => {
      if (before.has(button) === after.has(button)) return []
      return [{ button, pressed: after.has(button) }]
    })
  }
}
