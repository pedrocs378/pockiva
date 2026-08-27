import { z } from 'zod'

export const PROTOCOL_VERSION = 'v1' as const
export const MAX_SAFE_SEQUENCE = Number.MAX_SAFE_INTEGER

export const buttonSchema = z.enum(['up', 'down', 'left', 'right', 'a', 'b', 'start', 'select'])
export type Button = z.infer<typeof buttonSchema>

const sequenceSchema = z.int().nonnegative().max(MAX_SAFE_SEQUENCE)

const helloSchema = z
  .object({
    type: z.literal('hello'),
    version: z.literal(PROTOCOL_VERSION),
    token: z.string().min(1)
  })
  .strict()

const buttonDownSchema = z
  .object({
    type: z.literal('button-down'),
    button: buttonSchema,
    sequence: sequenceSchema
  })
  .strict()

const buttonUpSchema = z
  .object({
    type: z.literal('button-up'),
    button: buttonSchema,
    sequence: sequenceSchema
  })
  .strict()

const stateSyncSchema = z
  .object({
    type: z.literal('state-sync'),
    buttons: z
      .array(buttonSchema)
      .max(buttonSchema.options.length)
      .refine((buttons) => new Set(buttons).size === buttons.length, 'buttons must be unique'),
    sequence: sequenceSchema
  })
  .strict()

const pingSchema = z
  .object({
    type: z.literal('ping'),
    sequence: sequenceSchema
  })
  .strict()

export const clientMessageSchema = z.discriminatedUnion('type', [
  helloSchema,
  buttonDownSchema,
  buttonUpSchema,
  stateSyncSchema,
  pingSchema
])
export type ClientMessage = z.infer<typeof clientMessageSchema>

export const rejectionReasonSchema = z.enum([
  'invalid-token',
  'unsupported-version',
  'controller-already-connected',
  'malformed-message'
])

const welcomeSchema = z
  .object({
    type: z.literal('welcome'),
    version: z.literal(PROTOCOL_VERSION),
    controllerId: z.string().min(1)
  })
  .strict()

const rejectedSchema = z
  .object({
    type: z.literal('rejected'),
    reason: rejectionReasonSchema
  })
  .strict()

const pongSchema = z
  .object({
    type: z.literal('pong'),
    sequence: sequenceSchema
  })
  .strict()

const controllerDisconnectedSchema = z
  .object({
    type: z.literal('controller-disconnected')
  })
  .strict()

export const serverMessageSchema = z.discriminatedUnion('type', [
  welcomeSchema,
  rejectedSchema,
  pongSchema,
  controllerDisconnectedSchema
])
export type ServerMessage = z.infer<typeof serverMessageSchema>

export const parseClientMessage = (message: unknown): ClientMessage => clientMessageSchema.parse(message)

export const parseServerMessage = (message: unknown): ServerMessage => serverMessageSchema.parse(message)
