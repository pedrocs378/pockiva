import { describe, expect, it } from 'vitest'
import { parseRemoteEvent, parseRemoteSnapshot } from './remote-types'

const latency = { samples: 8, lastMs: 3, p95Ms: 7 }

describe('remote session native contract', () => {
  it.each([
    {
      phase: 'off',
      pairingUrl: null,
      expiresAtUnixMs: null,
      controllerId: null,
      latency: null,
      error: null
    },
    {
      phase: 'waiting',
      pairingUrl: 'http://192.168.1.10:4173/?token=secret',
      expiresAtUnixMs: 1_800_000_000_000,
      controllerId: null,
      latency,
      error: null
    },
    {
      phase: 'connected',
      pairingUrl: 'https://gameboy.local:4173/?token=secret',
      expiresAtUnixMs: 1_800_000_000_000,
      controllerId: 'controller-1',
      latency,
      error: null
    },
    {
      phase: 'error',
      pairingUrl: null,
      expiresAtUnixMs: null,
      controllerId: null,
      latency,
      error: { code: 'bind-failed', message: 'Port unavailable.' }
    }
  ])('accepts a strict $phase snapshot', (snapshot) => {
    expect(parseRemoteSnapshot(snapshot)).toEqual(snapshot)
    expect(parseRemoteEvent({ type: 'snapshot', snapshot })).toEqual({ type: 'snapshot', snapshot })
  })

  it.each([
    {
      phase: 'waiting',
      pairingUrl: null,
      expiresAtUnixMs: 1_800_000_000_000,
      controllerId: null,
      latency: null,
      error: null
    },
    {
      phase: 'connected',
      pairingUrl: 'http://192.168.1.10:4173/?token=secret',
      expiresAtUnixMs: 1_800_000_000_000,
      controllerId: null,
      latency: null,
      error: null
    },
    {
      phase: 'off',
      pairingUrl: 'http://192.168.1.10:4173/?token=leaked',
      expiresAtUnixMs: null,
      controllerId: null,
      latency: null,
      error: null
    },
    {
      phase: 'waiting',
      pairingUrl: 'ws://192.168.1.10:4173/controller',
      expiresAtUnixMs: 1_800_000_000_000,
      controllerId: null,
      latency: null,
      error: null
    },
    {
      phase: 'error',
      pairingUrl: null,
      expiresAtUnixMs: null,
      controllerId: null,
      latency: { samples: 1, lastMs: -1, p95Ms: 2 },
      error: { code: 'server-failed', message: 'Server stopped.' }
    },
    {
      phase: 'error',
      pairingUrl: null,
      expiresAtUnixMs: null,
      controllerId: null,
      latency: null,
      error: { code: 'unknown-code', message: 'Unknown.' }
    },
    {
      phase: 'unknown',
      pairingUrl: null,
      expiresAtUnixMs: null,
      controllerId: null,
      latency: null,
      error: null
    },
    {
      phase: 'off',
      pairingUrl: null,
      expiresAtUnixMs: null,
      controllerId: null,
      latency: null,
      error: null,
      unexpected: true
    }
  ])('rejects an invalid native snapshot', (snapshot) => {
    expect(() => parseRemoteSnapshot(snapshot)).toThrow()
  })
})
