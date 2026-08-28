export const SCREEN_WIDTH = 160
export const SCREEN_HEIGHT = 144
export const FRAME_HEADER_BYTE_LENGTH = 12
export const FRAME_RGBA_BYTE_LENGTH = 92_160
export const FRAME_PACKET_BYTE_LENGTH = 92_172

export type FramePacket = {
  sequence: number
  width: 160
  height: 144
  rgba: Uint8ClampedArray<ArrayBuffer>
}

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
    rgba: new Uint8ClampedArray(payload, FRAME_HEADER_BYTE_LENGTH, FRAME_RGBA_BYTE_LENGTH)
  }
}
