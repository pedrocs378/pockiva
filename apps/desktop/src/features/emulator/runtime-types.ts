import { z } from 'zod'

export const SCREEN_WIDTH = 160
export const SCREEN_HEIGHT = 144
export const FRAME_BYTE_LENGTH = SCREEN_WIDTH * SCREEN_HEIGHT * 4
export const FRAME_HEADER_BYTE_LENGTH = 12
export const FRAME_PACKET_BYTE_LENGTH = FRAME_HEADER_BYTE_LENGTH + FRAME_BYTE_LENGTH

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
export type FramePacket = {
  sequence: number
  width: typeof SCREEN_WIDTH
  height: typeof SCREEN_HEIGHT
  rgba: Uint8ClampedArray<ArrayBuffer>
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

export const decodeFramePacket = (payload: unknown): FramePacket => {
  if (!(payload instanceof ArrayBuffer)) {
    throw new TypeError('frame packet must be an ArrayBuffer')
  }
  if (payload.byteLength !== FRAME_PACKET_BYTE_LENGTH) {
    throw new RangeError(`frame packet must contain ${FRAME_PACKET_BYTE_LENGTH} bytes`)
  }

  const view = new DataView(payload)
  const rawSequence = view.getBigUint64(0, true)
  if (rawSequence > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new RangeError('frame sequence exceeds JavaScript safe integer range')
  }

  const width = view.getUint16(8, true)
  if (width !== SCREEN_WIDTH) {
    throw new RangeError(`frame width must be ${SCREEN_WIDTH} pixels`)
  }

  const height = view.getUint16(10, true)
  if (height !== SCREEN_HEIGHT) {
    throw new RangeError(`frame height must be ${SCREEN_HEIGHT} pixels`)
  }

  return {
    sequence: Number(rawSequence),
    width,
    height,
    rgba: new Uint8ClampedArray(payload, FRAME_HEADER_BYTE_LENGTH, FRAME_BYTE_LENGTH)
  }
}
