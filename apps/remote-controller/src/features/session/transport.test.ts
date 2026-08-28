import { describe, expect, it, vi } from 'vitest'
import { createWebSocketTransport, type WebSocketConstructor } from './transport'

describe('createWebSocketTransport', () => {
  it('forwards socket lifecycle and delegates send and close', () => {
    const listeners = new Map<string, EventListener>()
    const socket = {
      addEventListener: vi.fn((type: string, listener: EventListener) => listeners.set(type, listener)),
      removeEventListener: vi.fn(),
      send: vi.fn(),
      close: vi.fn()
    }
    const WebSocketImpl = vi.fn(function WebSocketMock() {
      return socket
    }) as unknown as WebSocketConstructor
    const handlers = { onOpen: vi.fn(), onMessage: vi.fn(), onClose: vi.fn(), onError: vi.fn() }

    const connection = createWebSocketTransport(WebSocketImpl).connect('ws://gb.local/controller', handlers)
    listeners.get('open')?.(new Event('open'))
    listeners.get('message')?.(new MessageEvent('message', { data: '{"type":"pong","sequence":1}' }))
    listeners.get('error')?.(new Event('error'))
    listeners.get('close')?.(new Event('close'))
    connection.send('hello')
    connection.close(1000, 'done')

    expect(WebSocketImpl).toHaveBeenCalledWith('ws://gb.local/controller')
    expect(handlers.onOpen).toHaveBeenCalledOnce()
    expect(handlers.onMessage).toHaveBeenCalledWith('{"type":"pong","sequence":1}')
    expect(handlers.onError).toHaveBeenCalledOnce()
    expect(handlers.onClose).toHaveBeenCalledOnce()
    expect(socket.send).toHaveBeenCalledWith('hello')
    expect(socket.close).toHaveBeenCalledWith(1000, 'done')
  })

  it('rejects non-text WebSocket frames at the transport boundary', () => {
    const listeners = new Map<string, EventListener>()
    const socket = {
      addEventListener: vi.fn((type: string, listener: EventListener) => listeners.set(type, listener)),
      removeEventListener: vi.fn(),
      send: vi.fn(),
      close: vi.fn()
    }
    const handlers = { onOpen: vi.fn(), onMessage: vi.fn(), onClose: vi.fn(), onError: vi.fn() }
    const WebSocketImpl = vi.fn(function WebSocketMock() {
      return socket
    }) as unknown as WebSocketConstructor

    createWebSocketTransport(WebSocketImpl).connect('ws://gb.local/controller', handlers)
    listeners.get('message')?.(new MessageEvent('message', { data: new Blob() }))

    expect(handlers.onMessage).not.toHaveBeenCalled()
    expect(handlers.onError).toHaveBeenCalledOnce()
  })
})
