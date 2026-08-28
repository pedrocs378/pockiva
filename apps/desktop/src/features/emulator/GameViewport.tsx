import { useEffect, useRef } from 'react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Skeleton } from '@/components/ui/skeleton'
import type { FramePacket, RuntimeError, RuntimePhase } from './runtime-types'

type GameViewportProps = {
  phase: RuntimePhase
  error: RuntimeError | null
  subscribeFrames: (consumer: (frame: FramePacket) => void) => () => void
  acknowledgeFrame: (sequence: number) => Promise<void>
}

export const GameViewport = ({ phase, error, subscribeFrames, acknowledgeFrame }: GameViewportProps) => {
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

      context.putImageData(new ImageData(frame.rgba, frame.width, frame.height), 0, 0)
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
      if (animationFrameRef.current !== null) cancelAnimationFrame(animationFrameRef.current)
      animationFrameRef.current = null
    }
  }, [acknowledgeFrame, subscribeFrames])

  return (
    <div className="game-viewport-shell">
      <canvas
        ref={canvasRef}
        aria-label="Game display"
        className="game-viewport-canvas"
        height={144}
        role="img"
        width={160}
      />

      {phase === 'empty' && (
        <div className="game-viewport-overlay">
          <strong>Open a ROM to begin</strong>
          <span>160 × 144</span>
        </div>
      )}

      {phase === 'loading' && (
        <div aria-label="Loading ROM" aria-live="polite" className="game-viewport-overlay" role="status">
          <Skeleton className="h-6 w-36 motion-reduce:animate-none" />
          <span>Loading ROM</span>
        </div>
      )}

      {phase === 'paused' && <div className="game-viewport-overlay game-viewport-overlay-muted">Paused</div>}

      {phase === 'error' && error && (
        <div className="game-viewport-overlay game-viewport-error">
          <Alert variant="destructive">
            <AlertDescription>{error.message}</AlertDescription>
          </Alert>
        </div>
      )}
    </div>
  )
}
