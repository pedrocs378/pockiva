import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { UpdatePrompt } from './UpdatePrompt'
import type { UpdaterView } from './use-updater'

afterEach(cleanup)

const view = (state: UpdaterView['state']): UpdaterView => ({
  state,
  dismiss: vi.fn().mockResolvedValue(undefined),
  install: vi.fn().mockResolvedValue(undefined)
})

describe('UpdatePrompt', () => {
  it('renders version and release notes and lets the user choose when to update', async () => {
    const user = userEvent.setup()
    const updater = view({ phase: 'available', version: '0.2.0', notes: 'Faster and more compatible.' })
    render(<UpdatePrompt updater={updater} />)

    expect(screen.getByRole('heading', { name: 'Pockiva 0.2.0 is available' })).toBeVisible()
    expect(screen.getByText('Faster and more compatible.')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Later' }))
    expect(updater.dismiss).toHaveBeenCalledOnce()

    await user.click(screen.getByRole('button', { name: 'Update now' }))
    expect(updater.install).toHaveBeenCalledOnce()
  })

  it('renders determinate download progress and locks dismissal while installing', () => {
    const updater = view({
      phase: 'downloading',
      version: '0.2.0',
      notes: null,
      progress: { downloadedBytes: 50, totalBytes: 100, percent: 50 }
    })
    render(<UpdatePrompt updater={updater} />)

    expect(screen.getByRole('progressbar', { name: 'Update download progress' })).toHaveAttribute('value', '50')
    expect(screen.getByText('Downloading update… 50%')).toBeVisible()
    expect(screen.queryByRole('button', { name: 'Later' })).not.toBeInTheDocument()
  })

  it('renders installation failures without exposing a dead-end dialog', async () => {
    const user = userEvent.setup()
    const updater = view({ phase: 'error', message: 'The update signature could not be verified.' })
    render(<UpdatePrompt updater={updater} />)

    expect(screen.getByRole('heading', { name: 'Update failed' })).toBeVisible()
    expect(screen.getByText('The update signature could not be verified.')).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Close' }))
    expect(updater.dismiss).toHaveBeenCalledOnce()
  })
})
