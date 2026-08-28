import { act, fireEvent, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { ControllerSession } from '@/features/session/controller-session'
import { MockControllerServer } from '@/test/mock-controller-server'
import { useControllerInput } from './use-controller-input'

const connectedSession = async () => {
  const server = new MockControllerServer()
  const session = new ControllerSession({
    pairing: { token: 'pairing-token', socketUrl: 'ws://gb.local/controller' },
    transport: server.createTransport()
  })
  session.connect()
  await Promise.resolve()
  return { session, server }
}

afterEach(() => {
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' })
})

describe('useControllerInput', () => {
  it('sends only changed transitions when a pointer moves between joystick sectors', async () => {
    const { session, server } = await connectedSession()
    const { result } = renderHook(() => useControllerInput(session))
    act(() => result.current.setPointerButtons(7, ['up', 'right']))
    act(() => result.current.setPointerButtons(7, ['right']))
    expect(server.receivedMessages.slice(-3)).toEqual([
      { type: 'button-down', button: 'up', sequence: 1 },
      { type: 'button-down', button: 'right', sequence: 2 },
      { type: 'button-up', button: 'up', sequence: 3 }
    ])
  })

  it('releases directions without releasing an action pointer', async () => {
    const { session, server } = await connectedSession()
    const { result } = renderHook(() => useControllerInput(session))
    act(() => {
      result.current.setPointerButtons(1, ['up', 'right'])
      result.current.pressPointer(2, 'a')
      result.current.releaseButtons(['up', 'down', 'left', 'right'])
    })
    expect(result.current.pressedButtons).toEqual(new Set(['a']))
    expect(server.receivedMessages.slice(-2)).toEqual([
      { type: 'button-up', button: 'up', sequence: 4 },
      { type: 'button-up', button: 'right', sequence: 5 }
    ])
  })

  it('clears all pointers and syncs empty state when the document becomes hidden', async () => {
    const { session, server } = await connectedSession()
    const { result } = renderHook(() => useControllerInput(session))
    act(() => {
      result.current.pressPointer(1, 'up')
      result.current.pressPointer(2, 'a')
    })
    expect(result.current.pressedButtons).toEqual(new Set(['up', 'a']))

    Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' })
    fireEvent(document, new Event('visibilitychange'))

    expect(result.current.pressedButtons).toEqual(new Set())
    expect(server.receivedMessages.at(-1)).toMatchObject({ type: 'state-sync', buttons: [] })
  })

  it('performs the same cleanup on pagehide and unmount', async () => {
    const { session, server } = await connectedSession()
    const { result, unmount } = renderHook(() => useControllerInput(session))
    act(() => result.current.pressPointer(4, 'b'))
    fireEvent(window, new Event('pagehide'))
    expect(server.receivedMessages.at(-1)).toMatchObject({ type: 'state-sync', buttons: [] })
    act(() => result.current.pressPointer(5, 'start'))
    unmount()
    expect(server.receivedMessages.at(-1)).toMatchObject({ type: 'state-sync', buttons: [] })
  })
})
