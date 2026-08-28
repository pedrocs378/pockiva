import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { GameViewport } from './GameViewport'
import type { FramePacket, RuntimeError, RuntimePhase } from './runtime-types'

const subscribeFrames = vi.fn()
const acknowledgeFrame = vi.fn().mockResolvedValue(undefined)
let frameConsumer: ((frame: FramePacket) => void) | null = null
let animationFrame: FrameRequestCallback | null = null
const putImageData = vi.fn()

const renderViewport = (phase: RuntimePhase, error: RuntimeError | null = null) =>
  render(
    <GameViewport phase={phase} error={error} subscribeFrames={subscribeFrames} acknowledgeFrame={acknowledgeFrame} />
  )

describe('GameViewport', () => {
  beforeEach(() => {
    frameConsumer = null
    animationFrame = null
    putImageData.mockReset()
    acknowledgeFrame.mockReset().mockResolvedValue(undefined)
    subscribeFrames.mockReset().mockImplementation((consumer: (frame: FramePacket) => void) => {
      frameConsumer = consumer
      return vi.fn()
    })
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ putImageData } as never)
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      animationFrame = callback
      return 1
    })
    vi.stubGlobal('cancelAnimationFrame', vi.fn())
    vi.stubGlobal(
      'ImageData',
      class {
        constructor(
          readonly data: Uint8ClampedArray,
          readonly width: number,
          readonly height: number
        ) {}
      }
    )
  })

  afterEach(() => {
    cleanup()
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('renders the empty state and native resolution', () => {
    renderViewport('empty')

    expect(screen.getByText('Open a ROM to begin')).toBeVisible()
    expect(screen.getByText('160 × 144')).toBeVisible()
  })

  it('renders loading, paused, and error overlays explicitly', () => {
    const { rerender } = render(
      <GameViewport
        phase="loading"
        error={null}
        subscribeFrames={subscribeFrames}
        acknowledgeFrame={acknowledgeFrame}
      />
    )
    expect(screen.getByLabelText('Loading ROM')).toBeVisible()

    rerender(
      <GameViewport phase="paused" error={null} subscribeFrames={subscribeFrames} acknowledgeFrame={acknowledgeFrame} />
    )
    expect(screen.getByText('Paused')).toBeVisible()

    rerender(
      <GameViewport
        phase="error"
        error={{ code: 'invalid-rom', message: 'This file is not a valid Game Boy ROM.' }}
        subscribeFrames={subscribeFrames}
        acknowledgeFrame={acknowledgeFrame}
      />
    )
    expect(screen.getByRole('alert')).toHaveTextContent('This file is not a valid Game Boy ROM.')
  })

  it('draws the latest raw frame before acknowledging it', async () => {
    renderViewport('running')
    const canvas = screen.getByRole('img', { name: 'Game display' })
    expect(canvas).toHaveAttribute('width', '160')
    expect(canvas).toHaveAttribute('height', '144')

    const frame: FramePacket = {
      sequence: 11,
      width: 160,
      height: 144,
      rgba: new Uint8ClampedArray(92_160)
    }
    act(() => frameConsumer?.(frame))
    expect(putImageData).not.toHaveBeenCalled()

    await act(async () => animationFrame?.(0))

    expect(putImageData).toHaveBeenCalledOnce()
    expect(acknowledgeFrame).toHaveBeenCalledWith(11)
    expect(putImageData.mock.invocationCallOrder[0]).toBeLessThan(
      acknowledgeFrame.mock.invocationCallOrder[0] ?? Number.POSITIVE_INFINITY
    )
  })

  it('coalesces queued frames into one animation frame', async () => {
    renderViewport('running')
    const first = {
      sequence: 1,
      width: 160,
      height: 144,
      rgba: new Uint8ClampedArray(92_160)
    } as FramePacket
    const latest = { ...first, sequence: 2 }

    act(() => {
      frameConsumer?.(first)
      frameConsumer?.(latest)
    })
    await act(async () => animationFrame?.(0))

    expect(putImageData).toHaveBeenCalledOnce()
    expect(acknowledgeFrame).toHaveBeenCalledWith(2)
  })
})
