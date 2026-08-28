import type { Button } from '@gameboy/protocol'

export type JoystickVector = { x: number; y: number }
export type JoystickResolution = { buttons: readonly Button[]; knob: JoystickVector }

const DEAD_ZONE_RATIO = 0.24

const sectorButtons = [
  ['right'],
  ['down', 'right'],
  ['down'],
  ['down', 'left'],
  ['left'],
  ['up', 'left'],
  ['up'],
  ['up', 'right']
] as const satisfies readonly (readonly Button[])[]

export const resolveJoystickVector = (vector: JoystickVector, maxDistance: number): JoystickResolution => {
  if (maxDistance <= 0) return { buttons: [], knob: { x: 0, y: 0 } }

  const distance = Math.hypot(vector.x, vector.y)
  const ratio = distance > maxDistance ? maxDistance / distance : 1
  const knob = { x: vector.x * ratio, y: vector.y * ratio }
  if (distance < maxDistance * DEAD_ZONE_RATIO) return { buttons: [], knob }

  const degrees = ((Math.atan2(vector.y, vector.x) * 180) / Math.PI + 360) % 360
  const sector = Math.floor((degrees + 22.5) / 45) % 8
  return { buttons: sectorButtons[sector], knob }
}
