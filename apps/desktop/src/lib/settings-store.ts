import { LazyStore } from '@tauri-apps/plugin-store'

export interface SettingsStore {
  get<T>(key: string): Promise<T | null | undefined>
  set(key: string, value: unknown): Promise<void>
  save(): Promise<void>
}

export const tauriSettingsStore: SettingsStore = new LazyStore('settings.json')
