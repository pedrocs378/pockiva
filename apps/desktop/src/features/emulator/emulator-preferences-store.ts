import { type SettingsStore, tauriSettingsStore } from '@/lib/settings-store'
import { defaultEmulatorPreferences, type EmulatorPreferences, parseEmulatorPreferences } from './emulator-preferences'

const SETTINGS_KEY = 'emulatorPreferencesV1'

export class EmulatorPreferencesRepository {
  private saveChain: Promise<void> = Promise.resolve()

  constructor(private readonly store: SettingsStore) {}

  async load(): Promise<EmulatorPreferences> {
    const saved = await this.store.get<unknown>(SETTINGS_KEY)
    if (saved === null || saved === undefined) return defaultEmulatorPreferences

    try {
      return parseEmulatorPreferences(saved)
    } catch {
      await this.save(defaultEmulatorPreferences)
      return defaultEmulatorPreferences
    }
  }

  save(preferences: EmulatorPreferences): Promise<void> {
    const validated = parseEmulatorPreferences(preferences)
    const operation = this.saveChain
      .catch(() => undefined)
      .then(async () => {
        await this.store.set(SETTINGS_KEY, validated)
        await this.store.save()
      })
    this.saveChain = operation
    return operation
  }
}

export const tauriEmulatorPreferencesRepository = new EmulatorPreferencesRepository(tauriSettingsStore)
