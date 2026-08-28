import { describe, expect, it } from 'vitest'
import { resolveJoystickVector } from './joystick-direction'

describe('resolveJoystickVector', () => {
  it('returns no buttons inside the dead zone', () => {
    expect(resolveJoystickVector({ x: 10, y: 0 }, 100)).toMatchObject({ buttons: [] })
  })

  it('activates a direction at the dead-zone boundary', () => {
    expect(resolveJoystickVector({ x: 24, y: 0 }, 100).buttons).toEqual(['right'])
  })

  it.each([
    [{ x: 100, y: 0 }, ['right']],
    [{ x: 0, y: 100 }, ['down']],
    [{ x: -100, y: 0 }, ['left']],
    [{ x: 0, y: -100 }, ['up']],
    [{ x: 100, y: -100 }, ['up', 'right']],
    [{ x: 100, y: 100 }, ['down', 'right']],
    [{ x: -100, y: 100 }, ['down', 'left']],
    [{ x: -100, y: -100 }, ['up', 'left']]
  ] as const)('maps vector %o to %o', (vector, buttons) => {
    expect(resolveJoystickVector(vector, 100).buttons).toEqual(buttons)
  })

  it('clamps the rendered knob to the maximum travel', () => {
    expect(resolveJoystickVector({ x: 300, y: 400 }, 100).knob).toEqual({ x: 60, y: 80 })
  })

  it('uses deterministic sectors on each side of the 22.5 degree boundary', () => {
    expect(resolveJoystickVector({ x: 100, y: -41 }, 100).buttons).toEqual(['right'])
    expect(resolveJoystickVector({ x: 100, y: -42 }, 100).buttons).toEqual(['up', 'right'])
  })

  it.each([0, -100])('returns no buttons when maximum travel is %s', (maxDistance) => {
    expect(resolveJoystickVector({ x: 100, y: 0 }, maxDistance)).toEqual({
      buttons: [],
      knob: { x: 0, y: 0 }
    })
  })
})
