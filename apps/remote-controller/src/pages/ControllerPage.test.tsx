import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { MockControllerServer } from '@/test/mock-controller-server'
import { ControllerPage } from './ControllerPage'

afterEach(() => {
  vi.useRealTimers()
  vi.clearAllMocks()
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' })
})

describe('ControllerPage', () => {
  it('asks for a QR pairing link when the token is missing', () => {
    render(
      <ControllerPage
        pairingUrl={new URL('http://gb.local/')}
        transport={new MockControllerServer().createTransport()}
      />
    )
    expect(screen.getByRole('status')).toHaveTextContent('Pairing link required')
    expect(screen.getByText('Scan the QR Code shown by the desktop app.')).toBeVisible()
    expect(screen.getByRole('button', { name: 'A' })).toBeDisabled()
  })

  it('connects to a simulated session and sends simultaneous A plus Up input', async () => {
    const server = new MockControllerServer()
    render(
      <ControllerPage
        pairingUrl={new URL('http://gb.local/?token=pairing-token')}
        transport={server.createTransport()}
      />
    )
    expect(screen.getByRole('status')).toHaveTextContent('Connecting')
    expect(await screen.findByText('Connected')).toBeVisible()
    const up = screen.getByRole('button', { name: 'Up' })
    const a = screen.getByRole('button', { name: 'A' })
    fireEvent.pointerDown(up, { pointerId: 1, pointerType: 'touch', button: 0 })
    fireEvent.pointerDown(a, { pointerId: 2, pointerType: 'touch', button: 0 })
    expect(up).toHaveAttribute('data-pressed', 'true')
    expect(a).toHaveAttribute('data-pressed', 'true')
    expect(server.receivedMessages.slice(-2)).toEqual([
      { type: 'button-down', button: 'up', sequence: 1 },
      { type: 'button-down', button: 'a', sequence: 2 }
    ])
  })

  it('disconnects, releases input, and reconnects only when requested', async () => {
    const server = new MockControllerServer()
    const user = userEvent.setup()
    render(
      <ControllerPage
        pairingUrl={new URL('http://gb.local/?token=pairing-token')}
        transport={server.createTransport()}
      />
    )
    await screen.findByText('Connected')
    fireEvent.pointerDown(screen.getByRole('button', { name: 'B' }), { pointerId: 3, pointerType: 'touch', button: 0 })
    await user.click(screen.getByRole('button', { name: 'Disconnect' }))
    expect(screen.getByRole('status')).toHaveTextContent('Disconnected')
    expect(server.receivedMessages.at(-1)).toMatchObject({ type: 'state-sync', buttons: [] })
    await user.click(screen.getByRole('button', { name: 'Connect' }))
    expect(await screen.findByText('Connected')).toBeVisible()
  })

  it.each([
    ['invalid-token', 'Pairing link expired'],
    ['unsupported-version', 'Protocol mismatch'],
    ['controller-already-connected', 'Another controller is connected'],
    ['malformed-message', 'Server unavailable']
  ] as const)('renders the %s rejection clearly', async (rejectionReason, label) => {
    const server = new MockControllerServer({ rejectionReason })
    render(
      <ControllerPage
        pairingUrl={new URL('http://gb.local/?token=pairing-token')}
        transport={server.createTransport()}
      />
    )
    expect(await screen.findByText(label)).toBeVisible()
  })
})
