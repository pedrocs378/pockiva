import { describe, expect, it } from 'vitest'
import { parseRuntimeEvent, parseRuntimeSnapshot } from './runtime-types'

describe('desktop runtime wire contracts', () => {
  it('parses the paused snapshot emitted after a ROM loads', () => {
    expect(
      parseRuntimeSnapshot({
        phase: 'paused',
        rom: {
          title: 'Test Cart',
          fileName: 'test.gb',
          romIdentity: 'sha256:test',
          mapper: 'rom-only',
          compatibility: 'dmg'
        },
        error: null
      })
    ).toMatchObject({ phase: 'paused', rom: { fileName: 'test.gb' }, error: null })
  })

  it('rejects frame bytes on the JSON control channel', () => {
    expect(() => parseRuntimeEvent({ type: 'frame', rgba: [0, 1, 2, 3] })).toThrow()
  })

  it('rejects unknown runtime error codes', () => {
    expect(() =>
      parseRuntimeSnapshot({
        phase: 'error',
        rom: null,
        error: { code: 'network-failed', message: 'not a desktop lifecycle error' }
      })
    ).toThrow()
  })
})
