import { describe, expect, it } from 'vitest'
import { parsePairingUrl } from './pairing'

describe('parsePairingUrl', () => {
  it('derives an unencrypted local controller socket from a QR URL', () => {
    expect(parsePairingUrl(new URL('http://192.168.1.23:7421/?token=pairing%20token'))).toEqual({
      status: 'ready',
      config: {
        token: 'pairing token',
        socketUrl: 'ws://192.168.1.23:7421/controller'
      }
    })
  })

  it('derives wss from an https QR URL and ignores unrelated query values', () => {
    expect(parsePairingUrl(new URL('https://gb.local/play?theme=dark&token=abc'))).toEqual({
      status: 'ready',
      config: { token: 'abc', socketUrl: 'wss://gb.local/controller' }
    })
  })

  it.each(['http://gb.local/', 'http://gb.local/?token=', 'http://gb.local/?token=%20%20'])(
    'rejects a missing or blank token in %s',
    (value) => expect(parsePairingUrl(new URL(value))).toEqual({ status: 'missing-token' })
  )

  it('rejects origins that cannot map to WebSocket', () => {
    expect(parsePairingUrl(new URL('file:///controller.html?token=abc'))).toEqual({ status: 'invalid-url' })
  })
})
