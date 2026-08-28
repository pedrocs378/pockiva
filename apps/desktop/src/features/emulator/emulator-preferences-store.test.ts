import { describe, expect, it, vi } from 'vitest'
import type { SettingsStore } from '@/lib/settings-store'
import { defaultEmulatorPreferences } from './emulator-preferences'
import { EmulatorPreferencesRepository } from './emulator-preferences-store'

const createStore = (value: unknown = null) => {
  const calls: string[] = []
  const store: SettingsStore = {
    get: vi.fn().mockResolvedValue(value),
    set: vi.fn(async () => {
      calls.push('set')
    }),
    save: vi.fn(async () => {
      calls.push('save')
    })
  }
  return { store, calls }
}

describe('EmulatorPreferencesRepository', () => {
  it('uses defaults when preferences are missing', async () => {
    const { store } = createStore()

    await expect(new EmulatorPreferencesRepository(store).load()).resolves.toEqual(defaultEmulatorPreferences)
    expect(store.set).not.toHaveBeenCalled()
  })

  it('loads valid persisted preferences', async () => {
    const saved = { volumePercent: 35, muted: true, displayScale: 4 }
    const { store } = createStore(saved)

    await expect(new EmulatorPreferencesRepository(store).load()).resolves.toEqual(saved)
  })

  it.each([
    ['non-object', 'loud'],
    ['volume above range', { volumePercent: 120, muted: false, displayScale: 3 }],
    ['fractional volume', { volumePercent: 50.5, muted: false, displayScale: 3 }],
    ['unknown scale', { volumePercent: 50, muted: false, displayScale: 5 }],
    ['missing mute', { volumePercent: 50, displayScale: 2 }]
  ])('repairs %s preferences with safe defaults', async (_case, value) => {
    const { store, calls } = createStore(value)

    await expect(new EmulatorPreferencesRepository(store).load()).resolves.toEqual(defaultEmulatorPreferences)
    expect(store.set).toHaveBeenCalledWith('emulatorPreferencesV1', defaultEmulatorPreferences)
    expect(calls).toEqual(['set', 'save'])
  })

  it('serializes saves so the newest value cannot be overwritten by an older write', async () => {
    let resolveFirstWrite!: () => void
    const firstWrite = new Promise<void>((resolve) => {
      resolveFirstWrite = resolve
    })
    const writes: unknown[] = []
    const store: SettingsStore = {
      get: vi.fn().mockResolvedValue(null),
      set: vi.fn(async (_key, value) => {
        writes.push(value)
        if (writes.length === 1) await firstWrite
      }),
      save: vi.fn().mockResolvedValue(undefined)
    }
    const repository = new EmulatorPreferencesRepository(store)
    const first = repository.save({ volumePercent: 20, muted: false, displayScale: 2 })
    const second = repository.save({ volumePercent: 80, muted: false, displayScale: 4 })

    await vi.waitFor(() => {
      expect(writes).toEqual([{ volumePercent: 20, muted: false, displayScale: 2 }])
    })
    resolveFirstWrite()
    await Promise.all([first, second])

    expect(writes).toEqual([
      { volumePercent: 20, muted: false, displayScale: 2 },
      { volumePercent: 80, muted: false, displayScale: 4 }
    ])
  })
})
