import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { EmulatorRuntimeClient, RuntimeSubscription } from '@/features/emulator/runtime-client'
import type { RuntimeErrorCode, RuntimeSnapshot } from '@/features/emulator/runtime-types'
import { defaultKeyboardMapping } from '@/features/keyboard/keyboard-mapping'
import { KeyboardMappingRepository } from '@/features/keyboard/keyboard-mapping-store'
import { EmulatorPage } from './EmulatorPage'

const emptySnapshot: RuntimeSnapshot = { phase: 'empty', rom: null, error: null }
const pausedSnapshot: RuntimeSnapshot = {
  phase: 'paused',
  rom: {
    title: 'Fixture',
    fileName: 'fixture.gb',
    romIdentity: 'fixture',
    mapper: 'rom-only',
    compatibility: 'dmg'
  },
  error: null
}
const runningSnapshot: RuntimeSnapshot = { ...pausedSnapshot, phase: 'running' }

const deferred = <T,>() => {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise
  })
  return { promise, resolve }
}

const createClient = (): EmulatorRuntimeClient => ({
  pickRom: vi.fn().mockResolvedValue('/private/fixture.gb'),
  subscribe: vi.fn(async (_handlers: RuntimeSubscription) => emptySnapshot),
  snapshot: vi.fn().mockResolvedValue(emptySnapshot),
  openRom: vi.fn().mockResolvedValue(pausedSnapshot),
  start: vi.fn().mockResolvedValue(runningSnapshot),
  pause: vi.fn().mockResolvedValue(pausedSnapshot),
  restart: vi.fn().mockResolvedValue(runningSnapshot),
  close: vi.fn().mockResolvedValue(emptySnapshot),
  setKeyboardInput: vi.fn().mockResolvedValue(undefined),
  acknowledgeFrame: vi.fn().mockResolvedValue(undefined)
})

const createRepository = (value = defaultKeyboardMapping) =>
  new KeyboardMappingRepository({
    get: vi.fn().mockResolvedValue(value),
    set: vi.fn().mockResolvedValue(undefined),
    save: vi.fn().mockResolvedValue(undefined)
  })

const renderPage = (
  runtimeClient: EmulatorRuntimeClient = createClient(),
  keyboardMappingRepository = createRepository()
) => render(<EmulatorPage runtimeClient={runtimeClient} keyboardMappingRepository={keyboardMappingRepository} />)

afterEach(() => cleanup())

describe('EmulatorPage lifecycle', () => {
  it('starts with only Open ROM enabled', () => {
    renderPage()

    expect(screen.getByRole('heading', { name: 'Game Boy' })).toBeVisible()
    expect(screen.getByText('No ROM loaded')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Open ROM' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Start' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Pause' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Restart' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Close ROM' })).toBeDisabled()
    expect(screen.getByText('Mobile controller is off')).toBeVisible()
    expect(screen.getByText('Remote protocol v1')).toBeVisible()
  })

  it('opens, starts, and closes a ROM with explicit enablement', async () => {
    const user = userEvent.setup()
    const client = createClient()
    renderPage(client)

    await user.click(screen.getByRole('button', { name: 'Open ROM' }))
    await screen.findByText('fixture.gb')
    expect(screen.getAllByText('Paused')).toHaveLength(2)
    expect(screen.getByRole('button', { name: 'Start' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Pause' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Restart' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Close ROM' })).toBeEnabled()

    await user.click(screen.getByRole('button', { name: 'Start' }))
    expect(await screen.findByText('Running')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Pause' })).toBeEnabled()

    await user.click(screen.getByRole('button', { name: 'Close ROM' }))
    expect(await screen.findByText('No ROM loaded')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Start' })).toBeDisabled()
  })

  it('disables every lifecycle action while loading', async () => {
    const user = userEvent.setup()
    const client = createClient()
    const pending = deferred<RuntimeSnapshot>()
    vi.mocked(client.openRom).mockReturnValueOnce(pending.promise)
    renderPage(client)

    await user.click(screen.getByRole('button', { name: 'Open ROM' }))
    expect(screen.getByLabelText('Loading ROM')).toBeVisible()
    for (const name of ['Open ROM', 'Start', 'Pause', 'Restart', 'Close ROM']) {
      expect(screen.getByRole('button', { name })).toBeDisabled()
    }

    pending.resolve(pausedSnapshot)
    await screen.findByText('fixture.gb')
  })

  it.each([
    ['file-inaccessible', 'The ROM file could not be read'],
    ['invalid-rom', 'This file is not a valid Game Boy ROM'],
    ['cgb-only', 'Game Boy Color-only cartridges are not supported'],
    ['unsupported-mapper', 'This cartridge mapper is not supported'],
    ['core-failure', 'The emulator core stopped'],
    ['invalid-lifecycle', 'That action is not available'],
    ['runtime-unavailable', 'The desktop runtime is unavailable']
  ] satisfies Array<[RuntimeErrorCode, string]>)('maps %s to an actionable heading', async (code, heading) => {
    const user = userEvent.setup()
    const client = createClient()
    vi.mocked(client.openRom).mockRejectedValueOnce({ code, message: 'Detailed failure.' })
    renderPage(client)

    await user.click(screen.getByRole('button', { name: 'Open ROM' }))

    expect(await screen.findByText(heading)).toBeVisible()
    expect(screen.getByRole('button', { name: 'Open ROM' })).toBeEnabled()
    await waitFor(() => expect(screen.getAllByText('Detailed failure.')).toHaveLength(2))
  })

  it('loads persisted controls into the dialog', async () => {
    const user = userEvent.setup()
    const repository = createRepository({ ...defaultKeyboardMapping, a: 'KeyQ' })
    renderPage(createClient(), repository)

    await user.click(screen.getByRole('button', { name: 'Keyboard controls' }))

    expect(await screen.findByRole('button', { name: 'A: Q' })).toBeVisible()
  })

  it('activates a saved remap and suspends gameplay while the dialog is open', async () => {
    const user = userEvent.setup()
    const client = createClient()
    vi.mocked(client.subscribe).mockResolvedValueOnce(runningSnapshot)
    renderPage(client)
    await screen.findByText('Running')

    await user.click(screen.getByRole('button', { name: 'Keyboard controls' }))
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyX', bubbles: true }))
    expect(client.setKeyboardInput).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: 'A: X' }))
    fireEvent.keyDown(screen.getByRole('button', { name: 'Press a key…' }), { code: 'KeyQ' })
    await user.click(screen.getByRole('button', { name: 'Save controls' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())

    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyQ', bubbles: true }))
    await waitFor(() => expect(client.setKeyboardInput).toHaveBeenLastCalledWith(['a']))
  })

  it('releases active gameplay input when pausing', async () => {
    const user = userEvent.setup()
    const client = createClient()
    vi.mocked(client.subscribe).mockResolvedValueOnce(runningSnapshot)
    renderPage(client)
    await screen.findByText('Running')
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyX', bubbles: true }))
    await waitFor(() => expect(client.setKeyboardInput).toHaveBeenLastCalledWith(['a']))

    await user.click(screen.getByRole('button', { name: 'Pause' }))

    await waitFor(() => expect(client.setKeyboardInput).toHaveBeenLastCalledWith([]))
  })

  it('falls back to defaults when settings cannot be loaded', async () => {
    const repository = createRepository()
    vi.spyOn(repository, 'load').mockRejectedValueOnce(new Error('settings unavailable'))

    renderPage(createClient(), repository)

    expect(await screen.findByText('Controls could not be loaded. Default keys are active.')).toBeVisible()
  })
})
