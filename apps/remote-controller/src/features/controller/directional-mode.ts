import { z } from 'zod'
import { browserStorage, type Storage } from '@/lib/storage'

const SETTINGS_KEY = 'directionalModeV1'
const directionalModeSchema = z.enum(['d-pad', 'joystick'])

export type DirectionalMode = z.infer<typeof directionalModeSchema>
export const defaultDirectionalMode: DirectionalMode = 'd-pad'

export class DirectionalModeRepository {
  constructor(private readonly storage: Storage) {}

  load(): DirectionalMode {
    try {
      const value = this.storage.read(SETTINGS_KEY)
      if (value === null) return defaultDirectionalMode

      const parsed = directionalModeSchema.safeParse(value)
      if (parsed.success) return parsed.data

      this.save(defaultDirectionalMode)
      return defaultDirectionalMode
    } catch {
      return defaultDirectionalMode
    }
  }

  save(mode: DirectionalMode): void {
    const validated = directionalModeSchema.parse(mode)

    try {
      this.storage.write(SETTINGS_KEY, validated)
    } catch {
      // Storage availability is optional; the current session remains usable.
    }
  }
}

export const browserDirectionalModeRepository = new DirectionalModeRepository(browserStorage)
