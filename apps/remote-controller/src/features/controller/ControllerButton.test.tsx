import { createEvent, fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ControllerButton } from './ControllerButton'

describe('ControllerButton', () => {
  it('captures a primary pointer and reports down/up once', () => {
    const onPress = vi.fn()
    const onRelease = vi.fn()
    render(
      <ControllerButton button="a" label="A" pressed={false} disabled={false} onPress={onPress} onRelease={onRelease} />
    )
    const button = screen.getByRole('button', { name: 'A' })
    button.setPointerCapture = vi.fn()
    button.hasPointerCapture = vi.fn(() => true)
    button.releasePointerCapture = vi.fn()
    fireEvent.pointerDown(button, { pointerId: 7, pointerType: 'touch', button: 0 })
    fireEvent.pointerUp(button, { pointerId: 7, pointerType: 'touch', button: 0 })
    expect(button.setPointerCapture).toHaveBeenCalledWith(7)
    expect(onPress).toHaveBeenCalledWith(7, 'a')
    expect(onRelease).toHaveBeenCalledWith(7)
  })

  it.each(['pointerCancel', 'lostPointerCapture'] as const)('releases on %s', (eventName) => {
    const onRelease = vi.fn()
    render(
      <ControllerButton button="left" label="Left" pressed disabled={false} onPress={vi.fn()} onRelease={onRelease} />
    )
    fireEvent[eventName](screen.getByRole('button', { name: 'Left' }), { pointerId: 9 })
    expect(onRelease).toHaveBeenCalledWith(9)
  })

  it('exposes immediate visual and accessible pressed state', () => {
    render(
      <ControllerButton button="start" label="Start" pressed disabled={false} onPress={vi.fn()} onRelease={vi.fn()} />
    )
    expect(screen.getByRole('button', { name: 'Start' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: 'Start' })).toHaveAttribute('data-pressed', 'true')
  })

  it('prevents the long-press context menu', () => {
    render(
      <ControllerButton
        button="select"
        label="Select"
        pressed={false}
        disabled={false}
        onPress={vi.fn()}
        onRelease={vi.fn()}
      />
    )
    const button = screen.getByRole('button', { name: 'Select' })
    const event = createEvent.contextMenu(button, { cancelable: true })
    fireEvent(button, event)
    expect(event.defaultPrevented).toBe(true)
  })
})
