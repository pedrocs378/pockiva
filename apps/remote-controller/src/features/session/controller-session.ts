import {
  type Button,
  type ClientMessage,
  MAX_SAFE_SEQUENCE,
  PROTOCOL_VERSION,
  parseServerMessage
} from '@gameboy/protocol'
import { BUTTON_ORDER, HEARTBEAT_INTERVAL_MS, HEARTBEAT_TIMEOUT_MS, RECONNECT_DELAYS_MS } from '@/constants/controller'
import type { PairingConfig } from './pairing'
import type { SessionConnection, SessionTransport } from './transport'

export type SessionStatus =
  | 'connecting'
  | 'connected'
  | 'disconnected'
  | 'expired-token'
  | 'incompatible-protocol'
  | 'controller-in-use'
  | 'server-unavailable'

export type SessionSnapshot = {
  status: SessionStatus
  controllerId: string | null
  pressedButtons: ReadonlySet<Button>
  reconnectAttempt: number
}

export type SessionScheduler = {
  setTimeout: (callback: () => void, delayMs: number) => ReturnType<typeof setTimeout>
  clearTimeout: (timer: ReturnType<typeof setTimeout>) => void
}

export type ControllerSessionOptions = {
  pairing: PairingConfig
  transport: SessionTransport
  scheduler?: SessionScheduler
  heartbeatIntervalMs?: number
  heartbeatTimeoutMs?: number
  reconnectDelaysMs?: readonly number[]
  initialSequence?: number
}

const defaultScheduler: SessionScheduler = {
  setTimeout: (callback, delayMs) => setTimeout(callback, delayMs),
  clearTimeout: (timer) => clearTimeout(timer)
}

const rejectionStatuses = {
  'invalid-token': 'expired-token',
  'unsupported-version': 'incompatible-protocol',
  'controller-already-connected': 'controller-in-use',
  'malformed-message': 'server-unavailable'
} as const satisfies Record<string, SessionStatus>

export class ControllerSession {
  private readonly pairing: PairingConfig
  private readonly transport: SessionTransport
  private readonly scheduler: SessionScheduler
  private readonly heartbeatIntervalMs: number
  private readonly heartbeatTimeoutMs: number
  private readonly reconnectDelaysMs: readonly number[]
  private readonly listeners = new Set<() => void>()

  private desiredButtons = new Set<Button>()
  private snapshot: SessionSnapshot = {
    status: 'disconnected',
    controllerId: null,
    pressedButtons: new Set(),
    reconnectAttempt: 0
  }
  private connection: SessionConnection | null = null
  private generation = 0
  private reconnectEnabled = false
  private retryIndex = 0
  private nextSequence: number
  private awaitedPong: number | null = null
  private nextPingTimer: ReturnType<typeof setTimeout> | null = null
  private pongDeadlineTimer: ReturnType<typeof setTimeout> | null = null
  private retryTimer: ReturnType<typeof setTimeout> | null = null

  constructor(options: ControllerSessionOptions) {
    const initialSequence = options.initialSequence ?? 0
    if (!Number.isSafeInteger(initialSequence) || initialSequence < 0) {
      throw new RangeError('initialSequence must be a non-negative safe integer')
    }

    this.pairing = options.pairing
    this.transport = options.transport
    this.scheduler = options.scheduler ?? defaultScheduler
    this.heartbeatIntervalMs = options.heartbeatIntervalMs ?? HEARTBEAT_INTERVAL_MS
    this.heartbeatTimeoutMs = options.heartbeatTimeoutMs ?? HEARTBEAT_TIMEOUT_MS
    this.reconnectDelaysMs = options.reconnectDelaysMs ?? RECONNECT_DELAYS_MS
    this.nextSequence = initialSequence
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  getSnapshot = (): SessionSnapshot => this.snapshot

  connect = (): void => {
    if (this.snapshot.status === 'connecting' || this.snapshot.status === 'connected') return

    this.reconnectEnabled = true
    this.retryIndex = 0
    this.cancelRetry()
    this.startConnection()
  }

  disconnect = (): void => {
    if (this.snapshot.status === 'connected') this.syncButtons([])
    else this.desiredButtons.clear()

    this.reconnectEnabled = false
    this.cancelAllTimers()
    const connection = this.connection
    this.connection = null
    connection?.close(1000, 'client-disconnect')
    this.generation += 1
    this.retryIndex = 0
    this.publish({ status: 'disconnected', controllerId: null, reconnectAttempt: 0 })
  }

  setButton = (button: Button, pressed: boolean): void => {
    const isPressed = this.desiredButtons.has(button)
    if (pressed === isPressed) return

    if (pressed) this.desiredButtons.add(button)
    else this.desiredButtons.delete(button)
    this.publish()

    if (this.snapshot.status === 'connected') {
      this.sendClient({
        type: pressed ? 'button-down' : 'button-up',
        button,
        sequence: this.takeSequence()
      })
    }
  }

  syncButtons = (buttons: readonly Button[]): void => {
    const requested = new Set(buttons)
    this.desiredButtons = new Set(BUTTON_ORDER.filter((button) => requested.has(button)))
    this.publish()
    if (this.snapshot.status === 'connected') this.sendStateSync()
  }

  private startConnection(): void {
    this.cancelHeartbeat()
    const generation = ++this.generation
    this.publish({ status: 'connecting', controllerId: null, reconnectAttempt: this.retryIndex })

    try {
      this.connection = this.transport.connect(this.pairing.socketUrl, {
        onOpen: () => this.handleOpen(generation),
        onMessage: (data) => this.handleMessage(generation, data),
        onClose: () => this.handleClose(generation),
        onError: () => this.handleTransportFailure(generation)
      })
    } catch {
      this.connection = null
      this.scheduleRetry(generation)
    }
  }

  private handleOpen(generation: number): void {
    if (!this.isCurrent(generation)) return
    this.sendClient({ type: 'hello', version: PROTOCOL_VERSION, token: this.pairing.token })
  }

  private handleMessage(generation: number, data: string): void {
    if (!this.isCurrent(generation)) return

    let message: ReturnType<typeof parseServerMessage>
    try {
      message = parseServerMessage(JSON.parse(data))
    } catch {
      this.failTerminal('incompatible-protocol', 1002, 'invalid-server-message')
      return
    }

    switch (message.type) {
      case 'welcome':
        this.retryIndex = 0
        this.publish({ status: 'connected', controllerId: message.controllerId, reconnectAttempt: 0 })
        if (this.sendStateSync()) this.scheduleNextPing()
        break
      case 'rejected':
        this.failTerminal(rejectionStatuses[message.reason], 1000, `rejected:${message.reason}`)
        break
      case 'pong':
        this.handlePong(message.sequence)
        break
      case 'controller-disconnected':
        this.closeForRecovery(4001, 'server-disconnected', generation)
        break
    }
  }

  private handlePong(sequence: number): void {
    if (this.snapshot.status !== 'connected' || sequence !== this.awaitedPong) return
    this.awaitedPong = null
    this.clearPongDeadline()
    this.scheduleNextPing()
  }

  private handleClose(generation: number): void {
    if (!this.isCurrent(generation)) return
    this.connection = null
    this.cancelHeartbeat()
    this.scheduleRetry(generation)
  }

  private handleTransportFailure(generation: number): void {
    if (!this.isCurrent(generation) || !this.reconnectEnabled) return
    const connection = this.connection
    this.connection = null
    connection?.close(1011, 'transport-error')
    this.scheduleRetry(generation)
  }

  private closeForRecovery(code: number, reason: string, generation: number): void {
    if (!this.isCurrent(generation) || !this.reconnectEnabled) return
    const connection = this.connection
    this.connection = null
    connection?.close(code, reason)
    this.scheduleRetry(generation)
  }

  private failTerminal(status: SessionStatus, code: number, reason: string): void {
    this.reconnectEnabled = false
    this.cancelAllTimers()
    const connection = this.connection
    this.connection = null
    this.publish({ status, controllerId: null })
    connection?.close(code, reason)
  }

  private scheduleRetry(generation: number): void {
    if (!this.isCurrent(generation) || !this.reconnectEnabled || this.retryTimer !== null) return
    this.cancelHeartbeat()

    const delayMs = this.reconnectDelaysMs[this.retryIndex]
    if (delayMs === undefined) {
      this.reconnectEnabled = false
      this.publish({ status: 'server-unavailable', controllerId: null, reconnectAttempt: this.retryIndex })
      return
    }

    this.retryIndex += 1
    this.publish({ status: 'connecting', controllerId: null, reconnectAttempt: this.retryIndex })
    this.retryTimer = this.scheduler.setTimeout(() => {
      this.retryTimer = null
      if (this.reconnectEnabled) this.startConnection()
    }, delayMs)
  }

  private scheduleNextPing(): void {
    if (this.snapshot.status !== 'connected') return
    if (this.nextPingTimer !== null) this.scheduler.clearTimeout(this.nextPingTimer)
    this.nextPingTimer = this.scheduler.setTimeout(() => {
      this.nextPingTimer = null
      if (this.snapshot.status !== 'connected') return

      const sequence = this.takeSequence()
      this.awaitedPong = sequence
      this.pongDeadlineTimer = this.scheduler.setTimeout(() => {
        this.pongDeadlineTimer = null
        if (this.awaitedPong === sequence) {
          this.awaitedPong = null
          this.closeForRecovery(4000, 'heartbeat-timeout', this.generation)
        }
      }, this.heartbeatTimeoutMs)
      this.sendClient({ type: 'ping', sequence })
    }, this.heartbeatIntervalMs)
  }

  private sendStateSync(): boolean {
    return this.sendClient({
      type: 'state-sync',
      buttons: BUTTON_ORDER.filter((button) => this.desiredButtons.has(button)),
      sequence: this.takeSequence()
    })
  }

  private sendClient(message: ClientMessage): boolean {
    if (!this.connection) return false
    try {
      this.connection.send(JSON.stringify(message))
      return true
    } catch {
      this.handleTransportFailure(this.generation)
      return false
    }
  }

  private takeSequence(): number {
    const sequence = this.nextSequence
    this.nextSequence = sequence === MAX_SAFE_SEQUENCE ? 0 : sequence + 1
    return sequence
  }

  private publish(overrides: Partial<Omit<SessionSnapshot, 'pressedButtons'>> = {}): void {
    this.snapshot = {
      ...this.snapshot,
      ...overrides,
      pressedButtons: new Set(this.desiredButtons)
    }
    for (const listener of this.listeners) listener()
  }

  private isCurrent(generation: number): boolean {
    return generation === this.generation
  }

  private cancelHeartbeat(): void {
    if (this.nextPingTimer !== null) this.scheduler.clearTimeout(this.nextPingTimer)
    this.nextPingTimer = null
    this.clearPongDeadline()
    this.awaitedPong = null
  }

  private clearPongDeadline(): void {
    if (this.pongDeadlineTimer !== null) this.scheduler.clearTimeout(this.pongDeadlineTimer)
    this.pongDeadlineTimer = null
  }

  private cancelRetry(): void {
    if (this.retryTimer !== null) this.scheduler.clearTimeout(this.retryTimer)
    this.retryTimer = null
  }

  private cancelAllTimers(): void {
    this.cancelHeartbeat()
    this.cancelRetry()
  }
}
