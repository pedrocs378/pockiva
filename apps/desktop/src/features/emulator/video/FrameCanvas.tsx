import { useEffect, useRef } from 'react'
import { type FramePacket, SCREEN_HEIGHT, SCREEN_WIDTH } from './frame-packet'

type FrameCanvasProps = {
  subscribeFrames: (consumer: (frame: FramePacket) => void) => () => void
  acknowledgeFrame: (sequence: number) => Promise<void>
}

const viewportStyle = {
  width: '100%',
  height: 'auto',
  aspectRatio: '10 / 9',
  imageRendering: 'pixelated'
} as const

export const FrameCanvas = ({ subscribeFrames, acknowledgeFrame }: FrameCanvasProps) => {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const pendingFrameRef = useRef<FramePacket | null>(null)
  const animationFrameRef = useRef<number | null>(null)

  useEffect(() => {
    const draw = () => {
      animationFrameRef.current = null
      const frame = pendingFrameRef.current
      pendingFrameRef.current = null
      const context = canvasRef.current?.getContext('2d')
      if (!frame || !context) return

      context.putImageData(new ImageData(frame.rgba, SCREEN_WIDTH, SCREEN_HEIGHT), 0, 0)
      void acknowledgeFrame(frame.sequence)
    }

    const unsubscribe = subscribeFrames((frame) => {
      pendingFrameRef.current = frame
      if (animationFrameRef.current === null) {
        animationFrameRef.current = requestAnimationFrame(draw)
      }
    })

    return () => {
      unsubscribe()
      pendingFrameRef.current = null
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current)
      }
      animationFrameRef.current = null
    }
  }, [acknowledgeFrame, subscribeFrames])

  return (
    <canvas
      ref={canvasRef}
      aria-label="Game display"
      height={SCREEN_HEIGHT}
      role="img"
      style={viewportStyle}
      width={SCREEN_WIDTH}
    />
  )
}
