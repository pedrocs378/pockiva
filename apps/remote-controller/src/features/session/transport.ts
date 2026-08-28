export type TransportHandlers = {
  onOpen: () => void
  onMessage: (data: string) => void
  onClose: () => void
  onError: () => void
}

export type SessionConnection = {
  send: (data: string) => void
  close: (code?: number, reason?: string) => void
}

export type SessionTransport = {
  connect: (url: string, handlers: TransportHandlers) => SessionConnection
}

type BrowserSocket = {
  addEventListener: (type: string, listener: EventListener) => void
  removeEventListener: (type: string, listener: EventListener) => void
  send: (data: string) => void
  close: (code?: number, reason?: string) => void
}

export type WebSocketConstructor = new (url: string) => BrowserSocket

export const createWebSocketTransport = (
  WebSocketImpl: WebSocketConstructor = WebSocket
): SessionTransport => ({
  connect: (url, handlers) => {
    const socket = new WebSocketImpl(url)
    const onOpen: EventListener = () => handlers.onOpen()
    const onMessage: EventListener = (event) => {
      if (event instanceof MessageEvent && typeof event.data === 'string') {
        handlers.onMessage(event.data)
        return
      }
      handlers.onError()
    }
    const onClose: EventListener = () => handlers.onClose()
    const onError: EventListener = () => handlers.onError()

    socket.addEventListener('open', onOpen)
    socket.addEventListener('message', onMessage)
    socket.addEventListener('close', onClose)
    socket.addEventListener('error', onError)

    return {
      send: (data) => socket.send(data),
      close: (code, reason) => socket.close(code, reason)
    }
  }
})
