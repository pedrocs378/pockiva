import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { DirectionalControl } from './DirectionalControl'

const defaultProps = {
  disabled: false,
  pressedButtons: new Set(['up'] as const),
  onModeChange: vi.fn(),
  pressPointer: vi.fn(),
  setPointerButtons: vi.fn(),
  releasePointer: vi.fn()
}

describe('DirectionalControl', () => {
  it('renders an accessible mode selector and the selected directional surface', async () => {
    const user = userEvent.setup()
    const onModeChange = vi.fn()
    render(<DirectionalControl {...defaultProps} mode="d-pad" onModeChange={onModeChange} />)

    expect(screen.getByRole('radiogroup', { name: 'Directional control' })).toBeVisible()
    expect(screen.getByRole('radio', { name: 'D-pad' })).toBeChecked()
    expect(screen.getByRole('radio', { name: 'Joystick' })).not.toBeChecked()
    const up = screen.getByRole('button', { name: 'Up' })
    expect(up).toHaveAttribute('data-pressed', 'true')
    fireEvent.pointerDown(up, { pointerId: 1, pointerType: 'touch', button: 0 })
    expect(defaultProps.pressPointer).toHaveBeenCalledWith(1, 'up')

    await user.click(screen.getByRole('radio', { name: 'Joystick' }))

    expect(onModeChange).toHaveBeenCalledWith('joystick')
  })

  it('renders the fixed joystick in joystick mode and forwards its input callbacks', () => {
    render(<DirectionalControl {...defaultProps} mode="joystick" />)

    const joystick = screen.getByRole('group', { name: 'Virtual joystick' })
    joystick.getBoundingClientRect = () => ({
      x: 0,
      y: 0,
      top: 0,
      left: 0,
      right: 200,
      bottom: 200,
      width: 200,
      height: 200,
      toJSON: () => ({})
    })
    expect(joystick).toBeVisible()
    expect(joystick).toHaveAttribute('aria-disabled', 'false')
    expect(screen.queryByRole('button', { name: 'Up' })).not.toBeInTheDocument()
    fireEvent.pointerDown(joystick, { pointerId: 2, pointerType: 'touch', clientX: 130, clientY: 70 })
    expect(defaultProps.setPointerButtons).toHaveBeenCalledWith(2, ['up', 'right'])
  })

  it('disables the active directional surface without disabling mode selection', () => {
    render(<DirectionalControl {...defaultProps} mode="d-pad" disabled />)

    expect(screen.getByRole('button', { name: 'Up' })).toBeDisabled()
    expect(screen.getByRole('radio', { name: 'Joystick' })).toBeEnabled()
  })
})
