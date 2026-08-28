import { z } from 'zod'
import { type RuntimeButton, runtimeButtons } from '@/features/emulator/runtime-types'

export type KeyboardMapping = Record<RuntimeButton, string>

export const defaultKeyboardMapping: KeyboardMapping = Object.freeze({
  up: 'ArrowUp',
  down: 'ArrowDown',
  left: 'ArrowLeft',
  right: 'ArrowRight',
  a: 'KeyX',
  b: 'KeyZ',
  start: 'Enter',
  select: 'ShiftRight'
})

export const reservedKeyboardCodes: ReadonlySet<string> = new Set([
  'Escape',
  'Tab',
  'MetaLeft',
  'MetaRight',
  'ControlLeft',
  'ControlRight',
  'AltLeft',
  'AltRight',
  ...Array.from({ length: 12 }, (_, index) => `F${index + 1}`)
])

const mappingSchema = z
  .strictObject(
    Object.fromEntries(runtimeButtons.map((button) => [button, z.string().min(1)])) as Record<
      RuntimeButton,
      z.ZodString
    >
  )
  .superRefine((mapping, context) => {
    const values = Object.values(mapping)
    if (values.some((code) => reservedKeyboardCodes.has(code))) {
      context.addIssue({ code: 'custom', message: 'That key is reserved for desktop controls.' })
    }
    if (new Set(values).size !== values.length) {
      context.addIssue({
        code: 'custom',
        message: 'Each key can be assigned to only one Game Boy button.'
      })
    }
  })

const buttonLabels: Record<RuntimeButton, string> = {
  up: 'Up',
  down: 'Down',
  left: 'Left',
  right: 'Right',
  a: 'A',
  b: 'B',
  start: 'Start',
  select: 'Select'
}

export const parseKeyboardMapping = (value: unknown): KeyboardMapping => mappingSchema.parse(value)

export const remapButton = (mapping: KeyboardMapping, button: RuntimeButton, code: string): KeyboardMapping => {
  if (reservedKeyboardCodes.has(code)) {
    throw new Error('That key is reserved for desktop controls.')
  }
  const assignedButton = runtimeButtons.find((candidate) => candidate !== button && mapping[candidate] === code)
  if (assignedButton) {
    throw new Error(`Key already assigned to ${buttonLabels[assignedButton]}.`)
  }
  return parseKeyboardMapping({ ...mapping, [button]: code })
}

export const getKeyboardCodeLabel = (code: string): string => {
  const labels: Record<string, string> = {
    ArrowUp: '↑',
    ArrowDown: '↓',
    ArrowLeft: '←',
    ArrowRight: '→',
    ShiftLeft: 'Left Shift',
    ShiftRight: 'Right Shift',
    Enter: 'Enter',
    Space: 'Space'
  }
  if (labels[code]) return labels[code]
  if (code.startsWith('Key') && code.length === 4) return code.slice(3)
  if (code.startsWith('Digit') && code.length === 6) return code.slice(5)
  return code
}

export const getRuntimeButtonLabel = (button: RuntimeButton): string => buttonLabels[button]
