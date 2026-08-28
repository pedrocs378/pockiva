import type { Button } from '@gameboy/protocol'

export const BUTTON_ORDER = [
  'up',
  'down',
  'left',
  'right',
  'a',
  'b',
  'start',
  'select'
] as const satisfies readonly Button[]

export const D_PAD_BUTTONS = ['up', 'left', 'right', 'down'] as const satisfies readonly Button[]
export const ACTION_BUTTONS = ['b', 'a'] as const satisfies readonly Button[]
export const MENU_BUTTONS = ['select', 'start'] as const satisfies readonly Button[]

export const BUTTON_LABELS: Record<Button, string> = {
  up: 'Up',
  down: 'Down',
  left: 'Left',
  right: 'Right',
  a: 'A',
  b: 'B',
  start: 'Start',
  select: 'Select'
}

export const HEARTBEAT_INTERVAL_MS = 5_000
export const HEARTBEAT_TIMEOUT_MS = 12_000
export const RECONNECT_DELAYS_MS = [0, 500, 1_000, 2_000, 5_000] as const
