import { z } from 'zod'

export type RemotePhase = 'off' | 'waiting' | 'connected' | 'error'
export type RemoteErrorCode =
  | 'no-lan-address'
  | 'bind-failed'
  | 'assets-unavailable'
  | 'server-failed'
  | 'runtime-unavailable'
  | 'invalid-lifecycle'

export type RemoteLatency = {
  samples: number
  lastMs: number
  p95Ms: number
}

export type RemoteError = {
  code: RemoteErrorCode
  message: string
}

export type RemoteSnapshot =
  | {
      phase: 'off'
      pairingUrl: null
      expiresAtUnixMs: null
      controllerId: null
      latency: null
      error: null
    }
  | {
      phase: 'waiting'
      pairingUrl: string
      expiresAtUnixMs: number
      controllerId: null
      latency: RemoteLatency | null
      error: null
    }
  | {
      phase: 'connected'
      pairingUrl: string
      expiresAtUnixMs: number
      controllerId: string
      latency: RemoteLatency | null
      error: null
    }
  | {
      phase: 'error'
      pairingUrl: null
      expiresAtUnixMs: null
      controllerId: null
      latency: RemoteLatency | null
      error: RemoteError
    }

export type RemoteEvent = { type: 'snapshot'; snapshot: RemoteSnapshot }

const safeUnsignedInteger = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER)

const pairingUrlSchema = z
  .string()
  .url()
  .refine((value) => {
    const protocol = new URL(value).protocol
    return protocol === 'http:' || protocol === 'https:'
  }, 'Pairing URL must use HTTP or HTTPS')

const remoteLatencySchema = z
  .object({
    samples: safeUnsignedInteger,
    lastMs: safeUnsignedInteger,
    p95Ms: safeUnsignedInteger
  })
  .strict()

const remoteErrorSchema = z
  .object({
    code: z.enum([
      'no-lan-address',
      'bind-failed',
      'assets-unavailable',
      'server-failed',
      'runtime-unavailable',
      'invalid-lifecycle'
    ]),
    message: z.string()
  })
  .strict()

const remoteSnapshotSchema = z.discriminatedUnion('phase', [
  z
    .object({
      phase: z.literal('off'),
      pairingUrl: z.null(),
      expiresAtUnixMs: z.null(),
      controllerId: z.null(),
      latency: z.null(),
      error: z.null()
    })
    .strict(),
  z
    .object({
      phase: z.literal('waiting'),
      pairingUrl: pairingUrlSchema,
      expiresAtUnixMs: safeUnsignedInteger,
      controllerId: z.null(),
      latency: remoteLatencySchema.nullable(),
      error: z.null()
    })
    .strict(),
  z
    .object({
      phase: z.literal('connected'),
      pairingUrl: pairingUrlSchema,
      expiresAtUnixMs: safeUnsignedInteger,
      controllerId: z.string().min(1),
      latency: remoteLatencySchema.nullable(),
      error: z.null()
    })
    .strict(),
  z
    .object({
      phase: z.literal('error'),
      pairingUrl: z.null(),
      expiresAtUnixMs: z.null(),
      controllerId: z.null(),
      latency: remoteLatencySchema.nullable(),
      error: remoteErrorSchema
    })
    .strict()
])

const remoteEventSchema = z
  .object({
    type: z.literal('snapshot'),
    snapshot: remoteSnapshotSchema
  })
  .strict()

export const parseRemoteError = (value: unknown): RemoteError => remoteErrorSchema.parse(value)

export const parseRemoteSnapshot = (value: unknown): RemoteSnapshot => remoteSnapshotSchema.parse(value)

export const parseRemoteEvent = (value: unknown): RemoteEvent => remoteEventSchema.parse(value)
