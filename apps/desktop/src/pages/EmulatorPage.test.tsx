import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { EmulatorPage } from './EmulatorPage'

describe('EmulatorPage', () => {
  it('shows the foundation empty state', () => {
    render(<EmulatorPage />)

    expect(screen.getByRole('heading', { name: 'Game Boy' })).toBeVisible()
    expect(screen.getByText('No ROM loaded')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Open ROM' })).toBeDisabled()
    expect(screen.getByText('Mobile controller is off')).toBeVisible()
  })
})
