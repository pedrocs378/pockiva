import { describe, expect, it, vi } from 'vitest'
import { Storage } from '@/lib/storage'
import { DirectionalModeRepository } from './directional-mode'

const backend = (value: string | null = null) => ({
  getItem: vi.fn(() => value),
  setItem: vi.fn()
})

describe('DirectionalModeRepository', () => {
  it('uses d-pad when no preference exists', () => {
    expect(new DirectionalModeRepository(new Storage(backend())).load()).toBe('d-pad')
  })

  it('restores a valid joystick preference', () => {
    expect(new DirectionalModeRepository(new Storage(backend('"joystick"'))).load()).toBe('joystick')
  })

  it.each(['not-json', '"unknown"', '{"mode":"joystick"}'])('repairs malformed value %s', (value) => {
    const raw = backend(value)
    expect(new DirectionalModeRepository(new Storage(raw)).load()).toBe('d-pad')
    expect(raw.setItem).toHaveBeenCalledWith('directionalModeV1', '"d-pad"')
  })

  it('keeps the current session usable when browser storage throws', () => {
    const raw = {
      getItem: vi.fn(() => {
        throw new Error('blocked')
      }),
      setItem: vi.fn(() => {
        throw new Error('blocked')
      })
    }
    const repository = new DirectionalModeRepository(new Storage(raw))

    expect(repository.load()).toBe('d-pad')
    expect(() => repository.save('joystick')).not.toThrow()
  })
})
