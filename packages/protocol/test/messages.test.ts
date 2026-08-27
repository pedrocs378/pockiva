import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { parseClientMessage, parseServerMessage } from '../src/index'

type ProtocolFixtures = {
  validClientMessages: unknown[]
  validServerMessages: unknown[]
  invalidMessages: unknown[]
}

const fixtures = JSON.parse(
  readFileSync(new URL('../fixtures/protocol-v1.json', import.meta.url), 'utf8')
) as ProtocolFixtures

describe('protocol v1 client messages', () => {
  it('accepts each client message variant', () => {
    expect(parseClientMessage({ type: 'hello', version: 'v1', token: 'abc' })).toEqual({
      type: 'hello',
      version: 'v1',
      token: 'abc'
    })
    expect(parseClientMessage({ type: 'button-down', button: 'a', sequence: 1 })).toMatchObject({ button: 'a' })
    expect(parseClientMessage({ type: 'button-up', button: 'start', sequence: 2 })).toMatchObject({ button: 'start' })
    expect(parseClientMessage({ type: 'state-sync', buttons: ['up', 'b'], sequence: 3 })).toMatchObject({
      buttons: ['up', 'b']
    })
    expect(parseClientMessage({ type: 'ping', sequence: 4 })).toMatchObject({ sequence: 4 })
  })

  it('rejects invalid buttons and protocol versions', () => {
    expect(() => parseClientMessage({ type: 'button-down', button: 'turbo', sequence: 3 })).toThrow()
    expect(() => parseClientMessage({ type: 'hello', version: 'v2', token: 'abc' })).toThrow()
  })

  it('accepts only JavaScript-safe integer sequences', () => {
    expect(parseClientMessage({ type: 'ping', sequence: Number.MAX_SAFE_INTEGER })).toMatchObject({
      sequence: Number.MAX_SAFE_INTEGER
    })
    expect(() => parseClientMessage({ type: 'ping', sequence: Number.MAX_SAFE_INTEGER + 1 })).toThrow()
  })
})

describe('canonical protocol fixtures', () => {
  it('accepts every valid fixture with its matching parser', () => {
    for (const message of fixtures.validClientMessages) {
      expect(parseClientMessage(message)).toEqual(message)
    }

    for (const message of fixtures.validServerMessages) {
      expect(parseServerMessage(message)).toEqual(message)
    }
  })

  it('rejects every invalid fixture', () => {
    for (const message of fixtures.invalidMessages) {
      expect(() => parseClientMessage(message)).toThrow()
      expect(() => parseServerMessage(message)).toThrow()
    }
  })
})

describe('protocol v1 server messages', () => {
  it('accepts each server message variant', () => {
    expect(parseServerMessage({ type: 'welcome', version: 'v1', controllerId: 'controller-1' })).toMatchObject({
      version: 'v1'
    })
    expect(parseServerMessage({ type: 'rejected', reason: 'invalid-token' })).toMatchObject({
      reason: 'invalid-token'
    })
    expect(parseServerMessage({ type: 'pong', sequence: 4 })).toMatchObject({ sequence: 4 })
    expect(parseServerMessage({ type: 'controller-disconnected' })).toEqual({ type: 'controller-disconnected' })
  })

  it('accepts only JavaScript-safe integer sequences', () => {
    expect(parseServerMessage({ type: 'pong', sequence: Number.MAX_SAFE_INTEGER })).toMatchObject({
      sequence: Number.MAX_SAFE_INTEGER
    })
    expect(() => parseServerMessage({ type: 'pong', sequence: Number.MAX_SAFE_INTEGER + 1 })).toThrow()
  })
})
