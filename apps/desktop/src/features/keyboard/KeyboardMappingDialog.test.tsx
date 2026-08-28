import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { KeyboardMappingDialog } from './KeyboardMappingDialog'
import { defaultKeyboardMapping } from './keyboard-mapping'

const renderDialog = (save = vi.fn().mockResolvedValue(undefined)) => {
  const onOpenChange = vi.fn()
  const onSave = vi.fn()
  render(
    <KeyboardMappingDialog
      open
      mapping={defaultKeyboardMapping}
      repository={{ save }}
      onOpenChange={onOpenChange}
      onSave={onSave}
    />
  )
  return { save, onOpenChange, onSave }
}

afterEach(() => cleanup())

describe('KeyboardMappingDialog', () => {
  it('renders all eight Game Boy controls', () => {
    renderDialog()

    for (const label of ['Up', 'Down', 'Left', 'Right', 'A', 'B', 'Start', 'Select']) {
      expect(screen.getByText(label)).toBeVisible()
    }
  })

  it('captures a physical code into the draft', async () => {
    const user = userEvent.setup()
    renderDialog()

    await user.click(screen.getByRole('button', { name: 'A: X' }))
    fireEvent.keyDown(screen.getByRole('button', { name: 'Press a key…' }), { code: 'KeyQ' })

    expect(screen.getByRole('button', { name: 'A: Q' })).toBeVisible()
  })

  it('reports duplicate and reserved bindings without closing', async () => {
    const user = userEvent.setup()
    const { onOpenChange } = renderDialog()
    await user.click(screen.getByRole('button', { name: 'A: X' }))
    fireEvent.keyDown(screen.getByRole('button', { name: 'Press a key…' }), { code: 'KeyQ' })

    await user.click(screen.getByRole('button', { name: 'B: Z' }))
    fireEvent.keyDown(screen.getByRole('button', { name: 'Press a key…' }), { code: 'KeyQ' })
    expect(screen.getByRole('alert')).toHaveTextContent('Key already assigned to A.')

    fireEvent.keyDown(screen.getByRole('button', { name: 'Press a key…' }), { code: 'Escape' })
    expect(screen.getByRole('alert')).toHaveTextContent('That key is reserved for desktop controls.')
    expect(onOpenChange).not.toHaveBeenCalledWith(false)
  })

  it('restores defaults in the draft', async () => {
    const user = userEvent.setup()
    renderDialog()
    await user.click(screen.getByRole('button', { name: 'A: X' }))
    fireEvent.keyDown(screen.getByRole('button', { name: 'Press a key…' }), { code: 'KeyQ' })

    await user.click(screen.getByRole('button', { name: 'Restore defaults' }))

    expect(screen.getByRole('button', { name: 'A: X' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'B: Z' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'Start: Enter' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'Select: Right Shift' })).toBeVisible()
  })

  it('persists a validated draft and closes', async () => {
    const user = userEvent.setup()
    const { save, onOpenChange, onSave } = renderDialog()
    await user.click(screen.getByRole('button', { name: 'A: X' }))
    fireEvent.keyDown(screen.getByRole('button', { name: 'Press a key…' }), { code: 'KeyQ' })

    await user.click(screen.getByRole('button', { name: 'Save controls' }))

    await waitFor(() => expect(save).toHaveBeenCalledWith({ ...defaultKeyboardMapping, a: 'KeyQ' }))
    expect(onSave).toHaveBeenCalledWith({ ...defaultKeyboardMapping, a: 'KeyQ' })
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  it('keeps the draft open after a persistence failure', async () => {
    const user = userEvent.setup()
    const save = vi.fn().mockRejectedValue(new Error('disk full'))
    const { onOpenChange } = renderDialog(save)
    await user.click(screen.getByRole('button', { name: 'A: X' }))
    fireEvent.keyDown(screen.getByRole('button', { name: 'Press a key…' }), { code: 'KeyQ' })

    await user.click(screen.getByRole('button', { name: 'Save controls' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('Controls could not be saved.')
    expect(screen.getByRole('button', { name: 'A: Q' })).toBeVisible()
    expect(onOpenChange).not.toHaveBeenCalledWith(false)
  })

  it('cancels and discards the draft', async () => {
    const user = userEvent.setup()
    const { onOpenChange, onSave } = renderDialog()
    await user.click(screen.getByRole('button', { name: 'A: X' }))
    fireEvent.keyDown(screen.getByRole('button', { name: 'Press a key…' }), { code: 'KeyQ' })

    await user.click(screen.getByRole('button', { name: 'Cancel' }))

    expect(onOpenChange).toHaveBeenCalledWith(false)
    expect(onSave).not.toHaveBeenCalled()
  })
})
