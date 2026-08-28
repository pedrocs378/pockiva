import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { SettingsStore } from '@/lib/settings-store'
import { defaultEmulatorPreferences, type EmulatorPreferences } from '@/features/emulator/emulator-preferences'
import { EmulatorPreferencesRepository } from '@/features/emulator/emulator-preferences-store'
import type { EmulatorRuntimeClient, RuntimeSubscription } from '@/features/emulator/runtime-client'
import type { RuntimeErrorCode, RuntimeSnapshot } from '@/features/emulator/runtime-types'
import { defaultKeyboardMapping } from '@/features/keyboard/keyboard-mapping'
import { KeyboardMappingRepository } from '@/features/keyboard/keyboard-mapping-store'
import type { RemoteSessionClient } from '@/features/remote-controller/remote-client'
import type { RemoteErrorCode, RemoteSnapshot } from '@/features/remote-controller/remote-types'
import { EmulatorPage } from './EmulatorPage'

const desktopStyles = readFileSync(resolve(process.cwd(), 'src/styles.css'), 'utf8')

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
const remoteOffSnapshot: RemoteSnapshot = {
  phase: 'off',
  pairingUrl: null,
  expiresAtUnixMs: null,
  controllerId: null,
  latency: null,
  error: null
}

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
  setAudioGain: vi.fn().mockResolvedValue(undefined),
  setKeyboardInput: vi.fn().mockResolvedValue(undefined),
  acknowledgeFrame: vi.fn().mockResolvedValue(undefined)
})

const createRepository = (value = defaultKeyboardMapping) =>
  new KeyboardMappingRepository({
    get: vi.fn().mockResolvedValue(value),
    set: vi.fn().mockResolvedValue(undefined),
    save: vi.fn().mockResolvedValue(undefined)
  })

const createPreferencesRepository = (value: EmulatorPreferences = defaultEmulatorPreferences) =>
  new EmulatorPreferencesRepository({
    get: vi.fn().mockResolvedValue(value),
    set: vi.fn().mockResolvedValue(undefined),
    save: vi.fn().mockResolvedValue(undefined)
  })

const createRemoteClient = (initial: RemoteSnapshot = remoteOffSnapshot): RemoteSessionClient => ({
  subscribe: vi.fn().mockResolvedValue(initial),
  snapshot: vi.fn().mockResolvedValue(initial),
  start: vi.fn().mockResolvedValue(remoteOffSnapshot),
  end: vi.fn().mockResolvedValue(remoteOffSnapshot)
})

const renderPage = (
  runtimeClient: EmulatorRuntimeClient = createClient(),
  keyboardMappingRepository = createRepository(),
  remoteSessionClient = createRemoteClient(),
  emulatorPreferencesRepository = createPreferencesRepository()
) =>
  render(
    <EmulatorPage
      runtimeClient={runtimeClient}
      keyboardMappingRepository={keyboardMappingRepository}
      remoteSessionClient={remoteSessionClient}
      emulatorPreferencesRepository={emulatorPreferencesRepository}
    />
  )

afterEach(() => cleanup())

describe('EmulatorPage lifecycle', () => {
  it('starts with only Open ROM enabled', () => {
    renderPage()

    expect(screen.getByRole('heading', { name: 'Game Boy' })).toBeVisible()
    expect(screen.getByText('No ROM loaded')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Open ROM' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Resume' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Restart' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Close ROM' })).toBeDisabled()
    expect(screen.getByText('Mobile controller is off')).toBeVisible()
    expect(screen.getByText('Protocol v1')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Start mobile controller' })).toBeEnabled()
  })

  it('keeps the remote session client independent from ROM lifecycle actions', async () => {
    const user = userEvent.setup()
    const runtimeClient = createClient()
    const remoteSessionClient = createRemoteClient()
    renderPage(runtimeClient, createRepository(), remoteSessionClient)

    await screen.findByText('Mobile controller is off')
    await user.click(screen.getByRole('button', { name: 'Start mobile controller' }))

    expect(remoteSessionClient.start).toHaveBeenCalledOnce()
    expect(runtimeClient.openRom).not.toHaveBeenCalled()
    expect(runtimeClient.start).not.toHaveBeenCalled()
  })

  it.each([
    ['no-lan-address', 'No local network address was found'],
    ['bind-failed', 'The controller server could not start'],
    ['assets-unavailable', 'The mobile controller files are unavailable'],
    ['server-failed', 'The controller session stopped'],
    ['runtime-unavailable', 'The emulator runtime is unavailable'],
    ['invalid-lifecycle', 'The controller session is busy']
  ] satisfies Array<[RemoteErrorCode, string]>)(
    'keeps ROM actions available during remote error %s',
    async (code, heading) => {
      const runtimeClient = createClient()
      vi.mocked(runtimeClient.subscribe).mockResolvedValueOnce(pausedSnapshot)
      const remoteError: RemoteSnapshot = {
        phase: 'error',
        pairingUrl: null,
        expiresAtUnixMs: null,
        controllerId: null,
        latency: null,
        error: { code, message: 'Detailed remote failure.' }
      }

      renderPage(runtimeClient, createRepository(), createRemoteClient(remoteError))

      expect(await screen.findByText(heading)).toBeVisible()
      for (const name of ['Open ROM', 'Resume', 'Restart', 'Close ROM', 'Keyboard controls']) {
        expect(screen.getByRole('button', { name })).toBeEnabled()
      }
    }
  )

  it('opens, starts, and closes a ROM with explicit enablement', async () => {
    const user = userEvent.setup()
    const client = createClient()
    renderPage(client)

    await user.click(screen.getByRole('button', { name: 'Open ROM' }))
    await screen.findByText('fixture.gb')
    expect(screen.getAllByText('Paused')).toHaveLength(2)
    expect(screen.getByRole('button', { name: 'Resume' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Restart' })).toBeEnabled()
    expect(screen.getByRole('button', { name: 'Close ROM' })).toBeEnabled()

    await user.click(screen.getByRole('button', { name: 'Resume' }))
    expect(await screen.findByText('Running')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Pause' })).toBeEnabled()

    await user.click(screen.getByRole('button', { name: 'Close ROM' }))
    expect(await screen.findByText('No ROM loaded')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Resume' })).toBeDisabled()
  })

  it('disables every lifecycle action while loading', async () => {
    const user = userEvent.setup()
    const client = createClient()
    const pending = deferred<RuntimeSnapshot>()
    vi.mocked(client.openRom).mockReturnValueOnce(pending.promise)
    renderPage(client)

    await user.click(screen.getByRole('button', { name: 'Open ROM' }))
    expect(screen.getByLabelText('Loading ROM')).toBeVisible()
    for (const name of ['Open ROM', 'Resume', 'Restart', 'Close ROM']) {
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

  it('loads persisted audio and display preferences and applies mute to the runtime', async () => {
    const client = createClient()
    const preferences = { volumePercent: 35, muted: true, displayScale: 4 } as const

    renderPage(client, createRepository(), createRemoteClient(), createPreferencesRepository(preferences))

    await waitFor(() => expect(client.setAudioGain).toHaveBeenCalledWith(0))
    expect(screen.getByLabelText('Volume')).toHaveValue('35')
    expect(screen.getByLabelText('Screen size')).toHaveValue('4')
    expect(screen.getByRole('button', { name: 'Unmute' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('img', { name: 'Game display' }).parentElement).toHaveStyle({
      '--game-screen-width': '640px'
    })
  })

  it('persists volume, mute, and display scale while applying changes immediately', async () => {
    const user = userEvent.setup()
    const client = createClient()
    const calls: string[] = []
    const store: SettingsStore = {
      get: vi.fn().mockResolvedValue(defaultEmulatorPreferences),
      set: vi.fn(async () => {
        calls.push('set')
      }),
      save: vi.fn(async () => {
        calls.push('save')
      })
    }
    renderPage(client, createRepository(), createRemoteClient(), new EmulatorPreferencesRepository(store))
    await waitFor(() => expect(client.setAudioGain).toHaveBeenCalledWith(1))

    await user.click(screen.getByRole('button', { name: 'Mute' }))
    expect(client.setAudioGain).toHaveBeenLastCalledWith(0)

    fireEvent.change(screen.getByLabelText('Volume'), { target: { value: '42' } })
    expect(client.setAudioGain).toHaveBeenLastCalledWith(0)

    await user.click(screen.getByRole('button', { name: 'Unmute' }))
    expect(client.setAudioGain).toHaveBeenLastCalledWith(0.42)

    await user.selectOptions(screen.getByLabelText('Screen size'), '4')
    await waitFor(() =>
      expect(store.set).toHaveBeenLastCalledWith('emulatorPreferencesV1', {
        volumePercent: 42,
        muted: false,
        displayScale: 4
      })
    )
    expect(calls.at(-1)).toBe('save')
  })

  it('does not let a delayed initial load replace a newer user preference', async () => {
    const client = createClient()
    const pending = deferred<EmulatorPreferences>()
    const repository = createPreferencesRepository()
    vi.spyOn(repository, 'load').mockReturnValueOnce(pending.promise)
    renderPage(client, createRepository(), createRemoteClient(), repository)

    fireEvent.change(screen.getByLabelText('Volume'), { target: { value: '42' } })
    expect(screen.getByLabelText('Volume')).toHaveValue('42')
    expect(client.setAudioGain).toHaveBeenLastCalledWith(0.42)

    await act(async () => {
      pending.resolve({ volumePercent: 15, muted: false, displayScale: 1 })
      await pending.promise
    })

    expect(screen.getByLabelText('Volume')).toHaveValue('42')
    expect(screen.getByLabelText('Screen size')).toHaveValue('3')
    expect(client.setAudioGain).toHaveBeenCalledOnce()
    expect(client.setAudioGain).toHaveBeenLastCalledWith(0.42)
  })

  it('provides desktop shortcuts without intercepting editable controls', async () => {
    const client = createClient()
    vi.mocked(client.subscribe).mockResolvedValueOnce(pausedSnapshot)
    renderPage(client)
    await screen.findByText('fixture.gb')

    fireEvent.keyDown(window, { code: 'Space' })
    await waitFor(() => expect(client.start).toHaveBeenCalledOnce())

    fireEvent.keyDown(window, { code: 'KeyR', metaKey: true, shiftKey: true })
    await waitFor(() => expect(client.restart).toHaveBeenCalledOnce())

    fireEvent.keyDown(screen.getByLabelText('Volume'), { code: 'Space' })
    expect(client.start).toHaveBeenCalledOnce()
  })

  it('falls back to defaults when settings cannot be loaded', async () => {
    const repository = createRepository()
    vi.spyOn(repository, 'load').mockRejectedValueOnce(new Error('settings unavailable'))

    renderPage(createClient(), repository)

    expect(await screen.findByText('Controls could not be loaded. Default keys are active.')).toBeVisible()
  })

  it('keeps defaults active when emulator preferences cannot be loaded', async () => {
    const repository = createPreferencesRepository()
    vi.spyOn(repository, 'load').mockRejectedValueOnce(new Error('settings unavailable'))

    renderPage(createClient(), createRepository(), createRemoteClient(), repository)

    expect(await screen.findByText(/Emulator preferences could not be/)).toBeVisible()
    expect(screen.getByLabelText('Volume')).toHaveValue('100')
    expect(screen.getByLabelText('Screen size')).toHaveValue('3')
  })

  it('collapses the remote controller layout at the 640px breakpoint', () => {
    expect(desktopStyles).toMatch(
      /@media\s*\(max-width:\s*40rem\)[\s\S]*?\.remote-controller-actions\s*\{[\s\S]*?flex-direction:\s*column;/
    )
  })
})
