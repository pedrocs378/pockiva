import { describe, expect, it } from 'vitest'
import { defaultKeyboardMapping, getKeyboardCodeLabel, parseKeyboardMapping, remapButton } from './keyboard-mapping'

describe('keyboard mapping', () => {
  it('defines one physical default for every Game Boy button', () => {
    expect(defaultKeyboardMapping).toEqual({
      up: 'ArrowUp',
      down: 'ArrowDown',
      left: 'ArrowLeft',
      right: 'ArrowRight',
      a: 'KeyX',
      b: 'KeyZ',
      start: 'Enter',
      select: 'ShiftRight'
    })
  })

  it('requires all eight buttons and rejects unknown buttons', () => {
    const { select: _select, ...incomplete } = defaultKeyboardMapping
    expect(() => parseKeyboardMapping(incomplete)).toThrow()
    expect(() => parseKeyboardMapping({ ...defaultKeyboardMapping, turbo: 'Space' })).toThrow()
  })

  it('rejects duplicate physical codes', () => {
    expect(() => parseKeyboardMapping({ ...defaultKeyboardMapping, b: defaultKeyboardMapping.a })).toThrow(
      'Each key can be assigned to only one Game Boy button.'
    )
  })

  it.each([
    'Escape',
    'Tab',
    'MetaLeft',
    'MetaRight',
    'ControlLeft',
    'ControlRight',
    'AltLeft',
    'AltRight',
    ...Array.from({ length: 12 }, (_, index) => `F${index + 1}`)
  ])('rejects reserved code %s', (code) => {
    expect(() => parseKeyboardMapping({ ...defaultKeyboardMapping, a: code })).toThrow(
      'That key is reserved for desktop controls.'
    )
  })

  it('remaps immutably', () => {
    const remapped = remapButton(defaultKeyboardMapping, 'a', 'KeyQ')

    expect(remapped.a).toBe('KeyQ')
    expect(defaultKeyboardMapping.a).toBe('KeyX')
  })

  it('rejects a remap already assigned to another button', () => {
    expect(() => remapButton(defaultKeyboardMapping, 'b', 'KeyX')).toThrow('Key already assigned to A.')
  })

  it.each([
    ['ArrowUp', '↑'],
    ['KeyX', 'X'],
    ['ShiftRight', 'Right Shift'],
    ['Enter', 'Enter']
  ])('labels %s as %s', (code, label) => {
    expect(getKeyboardCodeLabel(code)).toBe(label)
  })
})
