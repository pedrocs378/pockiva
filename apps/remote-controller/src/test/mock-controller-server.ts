import { type ClientMessage, PROTOCOL_VERSION, parseClientMessage, type ServerMessage } from '@gameboy/protocol'
import type { SessionConnection, SessionTransport, TransportHandlers } from '@/features/session/transport'

type RejectionReason = Extract<ServerMessage, { type: 'rejected' }>['reason']

export type MockControllerServerOptions = {
  validToken?: string
  rejectionReason?: RejectionReason
  autoPong?: boolean
  failConnections?: boolean
}

type ActiveConnection = {
  handlers: TransportHandlers
  close: () => void
}

export class MockControllerServer {
  readonly receivedMessages: ClientMessage[] = []
  connectionCount = 0
  autoPong: boolean
  failConnections: boolean

  private readonly validToken: string
  private readonly rejectionReason?: RejectionReason
  private activeConnection: ActiveConnection | null = null

  constructor(options: MockControllerServerOptions = {}) {
    this.validToken = options.validToken ?? 'pairing-token'
    this.rejectionReason = options.rejectionReason
    this.autoPong = options.autoPong ?? true
    this.failConnections = options.failConnections ?? false
  }

  createTransport(): SessionTransport {
    return {
      connect: (_url, handlers) => this.connect(handlers)
    }
  }

  dropConnection(): void {
    this.activeConnection?.close()
  }

  sendRaw(data: string): void {
    this.activeConnection?.handlers.onMessage(data)
  }

  private connect(handlers: TransportHandlers): SessionConnection {
    this.connectionCount += 1
    let closed = false
    const close = () => {
      if (closed) return
      closed = true
      if (this.activeConnection?.handlers === handlers) this.activeConnection = null
      handlers.onClose()
    }

    this.activeConnection = { handlers, close }
    queueMicrotask(() => {
      if (closed) return
      if (this.failConnections) {
        handlers.onError()
        close()
        return
      }
      handlers.onOpen()
    })

    return {
      send: (data) => {
        if (closed) throw new Error('mock connection is closed')
        const message = parseClientMessage(JSON.parse(data))
        this.receivedMessages.push(message)
        this.respond(message, handlers)
      },
      close: () => close()
    }
  }

  private respond(message: ClientMessage, handlers: TransportHandlers): void {
    if (message.type === 'hello') {
      if (this.rejectionReason) {
        this.send({ type: 'rejected', reason: this.rejectionReason }, handlers)
      } else if (message.token !== this.validToken) {
        this.send({ type: 'rejected', reason: 'invalid-token' }, handlers)
      } else {
        this.send({ type: 'welcome', version: PROTOCOL_VERSION, controllerId: 'controller-1' }, handlers)
      }
      return
    }

    if (message.type === 'ping' && this.autoPong) {
      this.send({ type: 'pong', sequence: message.sequence }, handlers)
    }
  }

  private send(message: ServerMessage, handlers: TransportHandlers): void {
    handlers.onMessage(JSON.stringify(message))
  }
}
