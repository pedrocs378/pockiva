import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { FrameCanvas, type FramePacket } from '.'

const subscribeFrames = vi.fn()
const acknowledgeFrame = vi.fn().mockResolvedValue(undefined)
const requestFrame = vi.fn()
const cancelFrame = vi.fn()
const getContext = vi.fn()
const putImageData = vi.fn()

let frameConsumer: ((frame: FramePacket) => void) | null = null
let animationFrame: FrameRequestCallback | null = null

class ImageDataStub {
  constructor(
    readonly data: Uint8ClampedArray,
    readonly width: number,
    readonly height: number
  ) {}
}

const createFrame = (sequence: number): FramePacket => ({
  sequence,
  width: 160,
  height: 144,
  rgba: new Uint8ClampedArray(92_160)
})

const renderCanvas = () => render(<FrameCanvas subscribeFrames={subscribeFrames} acknowledgeFrame={acknowledgeFrame} />)

describe('FrameCanvas', () => {
  beforeEach(() => {
    frameConsumer = null
    animationFrame = null
    putImageData.mockReset()
    getContext.mockReset().mockReturnValue({ putImageData })
    acknowledgeFrame.mockReset().mockResolvedValue(undefined)
    subscribeFrames.mockReset().mockImplementation((consumer: (frame: FramePacket) => void) => {
      frameConsumer = consumer
      return vi.fn()
    })
    requestFrame.mockReset().mockImplementation((callback: FrameRequestCallback) => {
      animationFrame = callback
      return 41
    })
    cancelFrame.mockReset()
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => getContext())
    vi.stubGlobal('requestAnimationFrame', requestFrame)
    vi.stubGlobal('cancelAnimationFrame', cancelFrame)
    vi.stubGlobal('ImageData', ImageDataStub)
  })

  afterEach(() => {
    cleanup()
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('uses the native DMG backing buffer and fixed pixelated scaling', () => {
    renderCanvas()

    const canvas = screen.getByRole('img', { name: 'Game display' }) as HTMLCanvasElement
    expect(canvas.width).toBe(160)
    expect(canvas.height).toBe(144)
    expect(canvas.style.width).toBe('100%')
    expect(canvas.style.height).toBe('auto')
    expect(canvas.style.aspectRatio).toBe('10 / 9')
    expect(canvas.style.imageRendering).toBe('pixelated')
  })

  it('draws one frame with source dimensions before acknowledging it', () => {
    renderCanvas()
    const frame = createFrame(11)

    act(() => frameConsumer?.(frame))
    expect(putImageData).not.toHaveBeenCalled()
    expect(acknowledgeFrame).not.toHaveBeenCalled()

    act(() => animationFrame?.(0))

    expect(putImageData).toHaveBeenCalledOnce()
    expect(putImageData).toHaveBeenCalledWith(
      expect.objectContaining({ data: frame.rgba, width: 160, height: 144 }),
      0,
      0
    )
    expect(acknowledgeFrame).toHaveBeenCalledExactlyOnceWith(11)
    expect(putImageData.mock.invocationCallOrder[0]).toBeLessThan(
      acknowledgeFrame.mock.invocationCallOrder[0] ?? Number.POSITIVE_INFINITY
    )
  })

  it('coalesces arrivals into one animation-frame draw of only the latest frame', () => {
    renderCanvas()
    const first = createFrame(1)
    const latest = createFrame(2)

    act(() => {
      frameConsumer?.(first)
      frameConsumer?.(latest)
    })

    expect(requestFrame).toHaveBeenCalledOnce()
    act(() => animationFrame?.(0))

    expect(putImageData).toHaveBeenCalledOnce()
    expect(putImageData).toHaveBeenCalledWith(expect.objectContaining({ data: latest.rgba }), 0, 0)
    expect(acknowledgeFrame).toHaveBeenCalledExactlyOnceWith(2)
  })

  it('cancels a pending animation frame on unmount without acknowledging an undrawn frame', () => {
    const { unmount } = renderCanvas()
    const frame = createFrame(7)

    act(() => frameConsumer?.(frame))
    const cancelledAnimationFrame = animationFrame
    unmount()

    expect(cancelFrame).toHaveBeenCalledExactlyOnceWith(41)
    act(() => cancelledAnimationFrame?.(0))
    expect(putImageData).not.toHaveBeenCalled()
    expect(acknowledgeFrame).not.toHaveBeenCalled()
  })

  it('does not throw or acknowledge when a 2D context is unavailable', () => {
    getContext.mockReturnValue(null)
    renderCanvas()

    act(() => frameConsumer?.(createFrame(9)))

    expect(() => act(() => animationFrame?.(0))).not.toThrow()
    expect(putImageData).not.toHaveBeenCalled()
    expect(acknowledgeFrame).not.toHaveBeenCalled()
  })
})
