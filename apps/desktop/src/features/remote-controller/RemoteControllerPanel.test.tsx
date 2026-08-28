import { act, cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { RemoteSessionClient } from './remote-client'
import type { RemoteErrorCode, RemoteSnapshot } from './remote-types'

const qrMock = vi.hoisted(() => vi.fn())

vi.mock('qrcode.react', () => ({
  QRCodeSVG: (props: Record<string, unknown>) => {
    qrMock(props)
    return <svg aria-label={String(props.title)} data-testid="pairing-qr" data-value={String(props.value)} />
  }
}))

import { RemoteControllerPanel } from './RemoteControllerPanel'

const offSnapshot: RemoteSnapshot = {
  phase: 'off',
  pairingUrl: null,
  expiresAtUnixMs: null,
  controllerId: null,
  latency: null,
  error: null
}

const waitingSnapshot: RemoteSnapshot = {
  phase: 'waiting',
  pairingUrl: 'http://192.168.1.10:4173/?token=secret',
  expiresAtUnixMs: Date.now() + 600_000,
  controllerId: null,
  latency: null,
  error: null
}

const connectedSnapshot: RemoteSnapshot = {
  ...waitingSnapshot,
  phase: 'connected',
  controllerId: 'controller-1'
}

const deferred = <T,>() => {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise
  })
  return { promise, resolve }
}

const createClient = (initial: RemoteSnapshot): RemoteSessionClient => ({
  subscribe: vi.fn(async () => initial),
  snapshot: vi.fn().mockResolvedValue(initial),
  start: vi.fn().mockResolvedValue(waitingSnapshot),
  end: vi.fn().mockResolvedValue(offSnapshot)
})

afterEach(() => {
  cleanup()
  qrMock.mockClear()
  vi.useRealTimers()
})

describe('RemoteControllerPanel', () => {
  it('starts an off session without exposing a QR or token', async () => {
    const user = userEvent.setup()
    const client = createClient(offSnapshot)
    render(<RemoteControllerPanel client={client} />)

    expect(await screen.findByText('Mobile controller is off')).toBeVisible()
    expect(screen.queryByTestId('pairing-qr')).not.toBeInTheDocument()
    expect(screen.queryByText(/token=/)).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Start mobile controller' }))
    expect(client.start).toHaveBeenCalledOnce()
  })

  it('renders the exact waiting URL in a QR with expiry and end action', async () => {
    const client = createClient(waitingSnapshot)
    render(<RemoteControllerPanel client={client} />)

    expect(await screen.findByText('Scan to connect')).toBeVisible()
    expect(screen.getByTestId('pairing-qr')).toHaveAttribute('data-value', waitingSnapshot.pairingUrl)
    expect(screen.getByRole('textbox', { name: 'Pairing URL' })).toHaveValue(waitingSnapshot.pairingUrl)
    expect(screen.getByText(/Pairing expires/)).toBeVisible()
    expect(screen.getByRole('button', { name: 'End session' })).toBeEnabled()
    expect(qrMock).toHaveBeenCalledWith(
      expect.objectContaining({
        value: waitingSnapshot.pairingUrl,
        size: 176,
        level: 'M',
        marginSize: 2,
        title: 'Mobile controller pairing QR Code'
      })
    )
  })

  it('renders connected identity without offering another pairing QR', async () => {
    const client = createClient(connectedSnapshot)
    render(<RemoteControllerPanel client={client} />)

    expect(await screen.findByText('Mobile controller connected')).toBeVisible()
    expect(screen.getByText('controller-1')).toBeVisible()
    expect(screen.getByRole('button', { name: 'End session' })).toBeEnabled()
    expect(screen.queryByTestId('pairing-qr')).not.toBeInTheDocument()
  })

  it('hides an expired waiting token while keeping an established controller connected', async () => {
    const expiredWaiting: RemoteSnapshot = { ...waitingSnapshot, expiresAtUnixMs: Date.now() - 1 }
    const { unmount } = render(<RemoteControllerPanel client={createClient(expiredWaiting)} />)

    expect(await screen.findByText('Pairing link expired')).toBeVisible()
    expect(screen.queryByTestId('pairing-qr')).not.toBeInTheDocument()
    expect(screen.queryByDisplayValue(expiredWaiting.pairingUrl)).not.toBeInTheDocument()
    unmount()

    const establishedAfterExpiry: RemoteSnapshot = {
      ...connectedSnapshot,
      expiresAtUnixMs: Date.now() - 1
    }
    render(<RemoteControllerPanel client={createClient(establishedAfterExpiry)} />)
    expect(await screen.findByText('Mobile controller connected')).toBeVisible()
  })

  it('updates a waiting session when its pairing deadline passes', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-08-28T12:00:00Z'))
    const expiring: RemoteSnapshot = {
      ...waitingSnapshot,
      expiresAtUnixMs: Date.now() + 1_000
    }
    render(<RemoteControllerPanel client={createClient(expiring)} />)
    await act(async () => Promise.resolve())
    expect(screen.getByText('Scan to connect')).toBeVisible()

    act(() => vi.advanceTimersByTime(1_001))

    expect(screen.getByText('Pairing link expired')).toBeVisible()
    expect(screen.queryByDisplayValue(expiring.pairingUrl)).not.toBeInTheDocument()
  })

  it('disables the end action while it is resolving', async () => {
    const user = userEvent.setup()
    const client = createClient(waitingSnapshot)
    const pending = deferred<RemoteSnapshot>()
    vi.mocked(client.end).mockReturnValueOnce(pending.promise)
    render(<RemoteControllerPanel client={client} />)
    const endButton = await screen.findByRole('button', { name: 'End session' })

    await user.click(endButton)

    expect(endButton).toBeDisabled()
    expect(endButton).toHaveTextContent('Ending session…')

    pending.resolve(offSnapshot)
    expect(await screen.findByRole('button', { name: 'Start mobile controller' })).toBeEnabled()
  })

  it.each([
    ['no-lan-address', 'No local network address was found'],
    ['bind-failed', 'The controller server could not start'],
    ['assets-unavailable', 'The mobile controller files are unavailable'],
    ['server-failed', 'The controller session stopped'],
    ['runtime-unavailable', 'The emulator runtime is unavailable'],
    ['invalid-lifecycle', 'The controller session is busy']
  ] satisfies Array<[RemoteErrorCode, string]>)('renders actionable copy for %s', async (code, heading) => {
    const snapshot: RemoteSnapshot = {
      phase: 'error',
      pairingUrl: null,
      expiresAtUnixMs: null,
      controllerId: null,
      latency: null,
      error: { code, message: 'Detailed remote failure.' }
    }
    render(<RemoteControllerPanel client={createClient(snapshot)} />)

    expect(await screen.findByText(heading)).toBeVisible()
    expect(screen.getByText('Detailed remote failure.')).toBeVisible()
    expect(screen.getByText(/keyboard/i)).toBeVisible()
    expect(screen.getByRole('button', { name: 'Try mobile controller again' })).toBeEnabled()
  })

  it('shows p95 latency only after at least one sample', async () => {
    const withLatency: RemoteSnapshot = {
      ...connectedSnapshot,
      latency: { samples: 1, lastMs: 5, p95Ms: 8 }
    }
    const { rerender } = render(<RemoteControllerPanel client={createClient(connectedSnapshot)} />)
    expect(await screen.findByText('Mobile controller connected')).toBeVisible()
    expect(screen.queryByText(/Local input p95/)).not.toBeInTheDocument()

    rerender(<RemoteControllerPanel client={createClient(withLatency)} />)
    expect(await screen.findByText('Local input p95: 8 ms')).toBeVisible()
  })
})
