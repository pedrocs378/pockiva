import { act, cleanup, fireEvent, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { RuntimeButton } from '@/features/emulator/runtime-types'
import { defaultKeyboardMapping, type KeyboardMapping } from './keyboard-mapping'
import { useKeyboardInput } from './use-keyboard-input'

const dispatchKey = (type: 'keydown' | 'keyup', code: string, options: { repeat?: boolean } = {}) => {
  const event = new KeyboardEvent(type, {
    bubbles: true,
    cancelable: true,
    code,
    repeat: options.repeat ?? false
  })
  window.dispatchEvent(event)
  return event
}

const renderInputHook = (
  setKeyboardInput = vi.fn<(buttons: RuntimeButton[]) => Promise<void>>().mockResolvedValue(undefined),
  mapping: KeyboardMapping = defaultKeyboardMapping
) => {
  const props = {
    mapping,
    enabled: true,
    suspended: false,
    setKeyboardInput
  }
  return { ...renderHook((nextProps) => useKeyboardInput(nextProps), { initialProps: props }), setKeyboardInput }
}

afterEach(() => cleanup())

describe('useKeyboardInput', () => {
  it('sends simultaneous buttons in canonical order and suppresses repeats', async () => {
    const { setKeyboardInput } = renderInputHook()

    const leftDown = dispatchKey('keydown', 'ArrowLeft')
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith(['left']))
    expect(leftDown.defaultPrevented).toBe(true)

    dispatchKey('keydown', 'KeyX')
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith(['left', 'a']))

    dispatchKey('keydown', 'KeyX', { repeat: true })
    expect(setKeyboardInput).toHaveBeenCalledTimes(2)

    dispatchKey('keyup', 'KeyX')
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith(['left']))
    dispatchKey('keyup', 'ArrowLeft')
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith([]))
  })

  it.each([
    ['ArrowLeft', 'left', defaultKeyboardMapping],
    ['Space', 'a', { ...defaultKeyboardMapping, a: 'Space' }]
  ] as const)('prevents repeated mapped %s input without duplicating IPC', async (code, button, mapping) => {
    const { setKeyboardInput } = renderInputHook(undefined, mapping)

    const initial = dispatchKey('keydown', code)
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith([button]))
    expect(initial.defaultPrevented).toBe(true)

    const repeated = dispatchKey('keydown', code, { repeat: true })
    await act(() => Promise.resolve())

    expect(repeated.defaultPrevented).toBe(true)
    expect(setKeyboardInput).toHaveBeenCalledTimes(1)
    dispatchKey('keyup', code)
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith([]))
  })

  it.each(Object.entries(defaultKeyboardMapping))('captures the default %s binding', async (button, code) => {
    const { setKeyboardInput, unmount } = renderInputHook()

    dispatchKey('keydown', code)
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith([button]))
    dispatchKey('keyup', code)
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith([]))
    unmount()
  })

  it.each(['input', 'textarea', 'select'])('ignores keydown from %s', async (tagName) => {
    const { setKeyboardInput } = renderInputHook()
    const element = document.createElement(tagName)
    document.body.append(element)

    fireEvent.keyDown(element, { code: 'KeyX' })
    await Promise.resolve()

    expect(setKeyboardInput).not.toHaveBeenCalled()
    element.remove()
  })

  it('ignores keydown from contenteditable', async () => {
    const { setKeyboardInput } = renderInputHook()
    const element = document.createElement('div')
    element.contentEditable = 'true'
    document.body.append(element)

    fireEvent.keyDown(element, { code: 'KeyX' })
    await Promise.resolve()

    expect(setKeyboardInput).not.toHaveBeenCalled()
    element.remove()
  })

  it('releases a held gameplay key even when keyup originates in an editor', async () => {
    const { setKeyboardInput } = renderInputHook()
    dispatchKey('keydown', 'KeyX')
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith(['a']))
    const input = document.createElement('input')
    document.body.append(input)

    fireEvent.keyUp(input, { code: 'KeyX' })

    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith([]))
    input.remove()
  })

  it('releases all buttons on blur and visibility loss', async () => {
    const { setKeyboardInput } = renderInputHook()
    dispatchKey('keydown', 'KeyX')
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith(['a']))

    fireEvent.blur(window)
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith([]))

    dispatchKey('keydown', 'KeyZ')
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith(['b']))
    Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' })
    document.dispatchEvent(new Event('visibilitychange'))
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith([]))
  })

  it('releases exactly once when disabled or unmounted', async () => {
    const { setKeyboardInput, rerender, unmount } = renderInputHook()
    dispatchKey('keydown', 'KeyX')
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith(['a']))

    rerender({
      mapping: defaultKeyboardMapping,
      enabled: false,
      suspended: false,
      setKeyboardInput
    })
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith([]))
    const callsAfterDisable = setKeyboardInput.mock.calls.length

    unmount()
    await act(() => Promise.resolve())
    expect(setKeyboardInput).toHaveBeenCalledTimes(callsAfterDisable)
  })

  it('releases when suspended and continues after a rejected write', async () => {
    const setKeyboardInput = vi
      .fn<(buttons: RuntimeButton[]) => Promise<void>>()
      .mockRejectedValueOnce(new Error('runtime busy'))
      .mockResolvedValue(undefined)
    const { rerender } = renderInputHook(setKeyboardInput)

    dispatchKey('keydown', 'ArrowLeft')
    dispatchKey('keyup', 'ArrowLeft')
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith([]))

    dispatchKey('keydown', 'KeyX')
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith(['a']))
    rerender({
      mapping: defaultKeyboardMapping,
      enabled: true,
      suspended: true,
      setKeyboardInput
    })
    await waitFor(() => expect(setKeyboardInput).toHaveBeenLastCalledWith([]))
  })
})
