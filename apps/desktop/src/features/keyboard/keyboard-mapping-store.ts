import { LazyStore } from '@tauri-apps/plugin-store'
import { defaultKeyboardMapping, type KeyboardMapping, parseKeyboardMapping } from './keyboard-mapping'

const SETTINGS_KEY = 'keyboardMappingV1'

export interface SettingsStore {
  get<T>(key: string): Promise<T | null | undefined>
  set(key: string, value: unknown): Promise<void>
  save(): Promise<void>
}

export class KeyboardMappingRepository {
  constructor(private readonly store: SettingsStore) {}

  async load(): Promise<KeyboardMapping> {
    const saved = await this.store.get<unknown>(SETTINGS_KEY)
    if (saved === null || saved === undefined) return defaultKeyboardMapping

    try {
      return parseKeyboardMapping(saved)
    } catch {
      await this.save(defaultKeyboardMapping)
      return defaultKeyboardMapping
    }
  }

  async save(mapping: KeyboardMapping): Promise<void> {
    const validated = parseKeyboardMapping(mapping)
    await this.store.set(SETTINGS_KEY, validated)
    await this.store.save()
  }
}

export const tauriKeyboardMappingRepository = new KeyboardMappingRepository(new LazyStore('settings.json'))
