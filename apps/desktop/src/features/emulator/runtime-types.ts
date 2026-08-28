import { z } from 'zod'

export const runtimeButtons = ['up', 'down', 'left', 'right', 'a', 'b', 'start', 'select'] as const
export type RuntimeButton = (typeof runtimeButtons)[number]
export type RuntimePhase = 'empty' | 'loading' | 'paused' | 'running' | 'error'
export type RuntimeErrorCode =
  | 'file-inaccessible'
  | 'invalid-rom'
  | 'cgb-only'
  | 'unsupported-mapper'
  | 'core-failure'
  | 'invalid-lifecycle'
  | 'runtime-unavailable'

export type RomSummary = {
  title: string
  fileName: string
  romIdentity: string
  mapper: 'rom-only' | 'mbc1' | 'mbc3' | 'mbc5'
  compatibility: 'dmg' | 'dmg-compatible'
}

export type RuntimeError = { code: RuntimeErrorCode; message: string }
export type RuntimeSnapshot = {
  phase: RuntimePhase
  rom: RomSummary | null
  error: RuntimeError | null
}
export type RuntimeEvent = { type: 'snapshot'; snapshot: RuntimeSnapshot }

const runtimeErrorSchema = z
  .object({
    code: z.enum([
      'file-inaccessible',
      'invalid-rom',
      'cgb-only',
      'unsupported-mapper',
      'core-failure',
      'invalid-lifecycle',
      'runtime-unavailable'
    ]),
    message: z.string()
  })
  .strict()

const romSummarySchema = z
  .object({
    title: z.string(),
    fileName: z.string(),
    romIdentity: z.string(),
    mapper: z.enum(['rom-only', 'mbc1', 'mbc3', 'mbc5']),
    compatibility: z.enum(['dmg', 'dmg-compatible'])
  })
  .strict()

const runtimeSnapshotSchema = z.discriminatedUnion('phase', [
  z.object({ phase: z.literal('empty'), rom: z.null(), error: z.null() }).strict(),
  z.object({ phase: z.literal('loading'), rom: z.null(), error: z.null() }).strict(),
  z.object({ phase: z.literal('paused'), rom: romSummarySchema, error: z.null() }).strict(),
  z.object({ phase: z.literal('running'), rom: romSummarySchema, error: z.null() }).strict(),
  z.object({ phase: z.literal('error'), rom: z.null(), error: runtimeErrorSchema }).strict()
])

const runtimeEventSchema = z
  .object({
    type: z.literal('snapshot'),
    snapshot: runtimeSnapshotSchema
  })
  .strict()

export const parseRuntimeError = (value: unknown): RuntimeError => runtimeErrorSchema.parse(value)

export const parseRuntimeSnapshot = (value: unknown): RuntimeSnapshot => runtimeSnapshotSchema.parse(value)

export const parseRuntimeEvent = (value: unknown): RuntimeEvent => runtimeEventSchema.parse(value)
