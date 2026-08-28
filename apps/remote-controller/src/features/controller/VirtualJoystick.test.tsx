import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { VirtualJoystick } from './VirtualJoystick'

const rectangle = {
  x: 0,
  y: 0,
  top: 0,
  left: 0,
  right: 200,
  bottom: 200,
  width: 200,
  height: 200,
  toJSON: () => ({})
}

const setup = (disabled = false) => {
  const setPointerButtons = vi.fn()
  const releasePointer = vi.fn()
  const view = render(
    <VirtualJoystick disabled={disabled} setPointerButtons={setPointerButtons} releasePointer={releasePointer} />
  )
  const joystick = screen.getByRole('group', { name: 'Virtual joystick' })
  joystick.getBoundingClientRect = () => rectangle
  joystick.setPointerCapture = vi.fn()
  joystick.hasPointerCapture = vi.fn(() => true)
  joystick.releasePointerCapture = vi.fn()
  return { ...view, joystick, setPointerButtons, releasePointer }
}

const pointerDown = (joystick: HTMLElement, pointerId = 4) => {
  fireEvent.pointerDown(joystick, {
    pointerId,
    pointerType: 'touch',
    clientX: 100,
    clientY: 100
  })
}

describe('VirtualJoystick', () => {
  it('captures one pointer and emits diagonal digital input', () => {
    const { joystick, setPointerButtons } = setup()

    pointerDown(joystick)
    fireEvent.pointerMove(joystick, {
      pointerId: 4,
      pointerType: 'touch',
      clientX: 130,
      clientY: 70
    })

    expect(joystick.setPointerCapture).toHaveBeenCalledWith(4)
    expect(setPointerButtons).toHaveBeenLastCalledWith(4, ['up', 'right'])
    expect(joystick).toHaveAttribute('data-directions', 'up right')
    expect(joystick.querySelector('.joystick-knob')).toHaveStyle({ transform: 'translate(30px, -30px)' })
  })

  it('releases the active directions when the pointer returns to the dead zone', () => {
    const { joystick, setPointerButtons } = setup()
    pointerDown(joystick)
    fireEvent.pointerMove(joystick, { pointerId: 4, pointerType: 'touch', clientX: 180, clientY: 100 })

    fireEvent.pointerMove(joystick, { pointerId: 4, pointerType: 'touch', clientX: 100, clientY: 100 })

    expect(setPointerButtons).toHaveBeenLastCalledWith(4, [])
    expect(joystick).toHaveAttribute('data-directions', '')
    expect(screen.getByText('Centered')).toBeInTheDocument()
  })

  it.each(['pointerUp', 'pointerCancel', 'lostPointerCapture'] as const)('releases once on %s', (eventName) => {
    const { joystick, releasePointer } = setup()
    pointerDown(joystick)

    fireEvent[eventName](joystick, { pointerId: 4, pointerType: 'touch' })
    fireEvent.lostPointerCapture(joystick, { pointerId: 4, pointerType: 'touch' })

    expect(releasePointer).toHaveBeenCalledTimes(1)
    expect(releasePointer).toHaveBeenCalledWith(4)
    expect(joystick).toHaveAttribute('data-directions', '')
  })

  it('ignores a second pointer while the first owns the joystick', () => {
    const { joystick, setPointerButtons } = setup()
    pointerDown(joystick, 4)

    pointerDown(joystick, 5)
    fireEvent.pointerMove(joystick, {
      pointerId: 5,
      pointerType: 'touch',
      clientX: 180,
      clientY: 100
    })

    expect(joystick.setPointerCapture).toHaveBeenCalledTimes(1)
    expect(setPointerButtons).not.toHaveBeenCalledWith(5, expect.anything())
  })

  it('ignores pointer input while disabled', () => {
    const { joystick, setPointerButtons, releasePointer } = setup(true)

    pointerDown(joystick)
    fireEvent.pointerMove(joystick, { pointerId: 4, pointerType: 'touch', clientX: 180, clientY: 100 })

    expect(joystick).toHaveAttribute('aria-disabled', 'true')
    expect(joystick.setPointerCapture).not.toHaveBeenCalled()
    expect(setPointerButtons).not.toHaveBeenCalled()
    expect(releasePointer).not.toHaveBeenCalled()
  })

  it('releases an active pointer when it becomes disabled', () => {
    const setPointerButtons = vi.fn()
    const releasePointer = vi.fn()
    const { rerender } = render(
      <VirtualJoystick disabled={false} setPointerButtons={setPointerButtons} releasePointer={releasePointer} />
    )
    const joystick = screen.getByRole('group', { name: 'Virtual joystick' })
    joystick.getBoundingClientRect = () => rectangle
    joystick.setPointerCapture = vi.fn()
    pointerDown(joystick)
    fireEvent.pointerMove(joystick, { pointerId: 4, pointerType: 'touch', clientX: 180, clientY: 100 })

    rerender(<VirtualJoystick disabled setPointerButtons={setPointerButtons} releasePointer={releasePointer} />)

    expect(releasePointer).toHaveBeenCalledTimes(1)
    expect(releasePointer).toHaveBeenCalledWith(4)
    expect(joystick).toHaveAttribute('data-directions', '')
  })

  it('releases an active pointer exactly once during unmount', () => {
    const { joystick, releasePointer, unmount } = setup()
    pointerDown(joystick)

    unmount()

    expect(releasePointer).toHaveBeenCalledTimes(1)
    expect(releasePointer).toHaveBeenCalledWith(4)
  })

  it('does not release a completed pointer again during unmount', () => {
    const { joystick, releasePointer, unmount } = setup()
    pointerDown(joystick)
    fireEvent.pointerUp(joystick, { pointerId: 4, pointerType: 'touch' })

    unmount()

    expect(releasePointer).toHaveBeenCalledTimes(1)
  })
})
