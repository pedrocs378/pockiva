import { describe, expect, it } from 'vitest'
import { decodeFramePacket, FRAME_PACKET_BYTE_LENGTH } from '.'

const createFramePacket = ({
  sequence = 3n,
  width = 160,
  height = 144
}: {
  sequence?: bigint
  width?: number
  height?: number
} = {}) => {
  const buffer = new ArrayBuffer(FRAME_PACKET_BYTE_LENGTH)
  const view = new DataView(buffer)
  view.setBigUint64(0, sequence, true)
  view.setUint16(8, width, true)
  view.setUint16(10, height, true)
  return buffer
}

describe('frame packet decoder', () => {
  it('decodes the stable little-endian binary frame header', () => {
    expect(decodeFramePacket(createFramePacket())).toMatchObject({
      sequence: 3,
      width: 160,
      height: 144,
      rgba: { byteLength: 92_160 }
    })
  })

  it('rejects a raw frame with the wrong byte length', () => {
    expect(() => decodeFramePacket(new ArrayBuffer(FRAME_PACKET_BYTE_LENGTH - 1))).toThrow(
      'frame packet must contain 92172 bytes'
    )
  })

  it.each([
    ['width', createFramePacket({ width: 159 }), 'frame width must be 160 pixels'],
    ['height', createFramePacket({ height: 143 }), 'frame height must be 144 pixels'],
    [
      'sequence',
      createFramePacket({ sequence: BigInt(Number.MAX_SAFE_INTEGER) + 1n }),
      'frame sequence exceeds JavaScript safe integer range'
    ]
  ])('rejects an invalid %s header', (_field, packet, message) => {
    expect(() => decodeFramePacket(packet)).toThrow(message)
  })

  it.each([
    ['JSON array', [0, 1, 2, 3]],
    ['SharedArrayBuffer', new SharedArrayBuffer(FRAME_PACKET_BYTE_LENGTH)],
    ['Uint8Array', new Uint8Array(createFramePacket())],
    ['string', 'raw frame bytes'],
    ['base64 string', 'AAAAAAAAAAAAAAAA']
  ])('rejects a %s payload', (_kind, payload) => {
    expect(() => decodeFramePacket(payload)).toThrow('frame packet must be an ArrayBuffer')
  })

  it('exposes RGBA bytes as a zero-copy view', () => {
    const packet = createFramePacket()
    const bytes = new Uint8Array(packet)
    bytes[12] = 17
    bytes[bytes.length - 1] = 239

    const frame = decodeFramePacket(packet)

    expect(frame.rgba[0]).toBe(17)
    expect(frame.rgba[frame.rgba.length - 1]).toBe(239)
    bytes[12] = 99
    expect(frame.rgba[0]).toBe(99)
  })
})
