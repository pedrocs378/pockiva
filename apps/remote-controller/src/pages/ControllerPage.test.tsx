import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ControllerPage } from './ControllerPage'

describe('ControllerPage', () => {
  it('shows every control in the disconnected foundation state', () => {
    render(<ControllerPage />)

    expect(screen.getByRole('heading', { name: 'Game Boy Controller' })).toBeVisible()
    expect(screen.getByText('Disconnected')).toBeVisible()

    for (const label of ['Up', 'Down', 'Left', 'Right', 'A', 'B', 'Start', 'Select']) {
      expect(screen.getByRole('button', { name: label })).toBeVisible()
    }

    expect(screen.getByText('Protocol v1')).toBeVisible()
  })
})
