import { beforeEach, describe, expect, it, vi } from 'vitest'
import { defaultKeyboardMapping } from './keyboard-mapping'
import { KeyboardMappingRepository, type SettingsStore } from './keyboard-mapping-store'

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

describe('KeyboardMappingRepository', () => {
  beforeEach(() => vi.restoreAllMocks())

  it('loads defaults when settings are missing', async () => {
    const { store } = createStore()
    const repository = new KeyboardMappingRepository(store)

    await expect(repository.load()).resolves.toEqual(defaultKeyboardMapping)
    expect(store.set).not.toHaveBeenCalled()
  })

  it('loads a valid saved mapping exactly', async () => {
    const saved = { ...defaultKeyboardMapping, a: 'KeyQ' }
    const { store } = createStore(saved)

    await expect(new KeyboardMappingRepository(store).load()).resolves.toEqual(saved)
  })

  it.each([
    ['corrupt', 'not-an-object'],
    ['incomplete', { up: 'ArrowUp' }],
    ['duplicate', { ...defaultKeyboardMapping, b: 'KeyX' }],
    ['reserved', { ...defaultKeyboardMapping, a: 'Escape' }]
  ])('repairs %s saved settings with defaults', async (_case, value) => {
    const { store, calls } = createStore(value)

    await expect(new KeyboardMappingRepository(store).load()).resolves.toEqual(defaultKeyboardMapping)
    expect(store.set).toHaveBeenCalledWith('keyboardMappingV1', defaultKeyboardMapping)
    expect(calls).toEqual(['set', 'save'])
  })

  it('validates and persists a mapping in order', async () => {
    const { store, calls } = createStore()
    const mapping = { ...defaultKeyboardMapping, a: 'KeyQ' }

    await new KeyboardMappingRepository(store).save(mapping)

    expect(store.set).toHaveBeenCalledWith('keyboardMappingV1', mapping)
    expect(calls).toEqual(['set', 'save'])
  })

  it('rejects when the settings store cannot be read', async () => {
    const { store } = createStore()
    vi.mocked(store.get).mockRejectedValueOnce(new Error('store unavailable'))

    await expect(new KeyboardMappingRepository(store).load()).rejects.toThrow('store unavailable')
  })

  it('never accesses localStorage', async () => {
    const localStorageGet = vi.spyOn(Storage.prototype, 'getItem')
    const { store } = createStore()

    await new KeyboardMappingRepository(store).load()

    expect(localStorageGet).not.toHaveBeenCalled()
  })
})
