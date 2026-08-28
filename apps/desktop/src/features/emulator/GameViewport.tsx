import { Alert, AlertDescription } from '@/components/ui/alert'
import { Skeleton } from '@/components/ui/skeleton'
import type { RuntimeError, RuntimePhase } from './runtime-types'
import { FrameCanvas, type FramePacket } from './video'

type GameViewportProps = {
  phase: RuntimePhase
  error: RuntimeError | null
  subscribeFrames: (consumer: (frame: FramePacket) => void) => () => void
  acknowledgeFrame: (sequence: number) => Promise<void>
}

export const GameViewport = ({ phase, error, subscribeFrames, acknowledgeFrame }: GameViewportProps) => {
  return (
    <div className="game-viewport-shell">
      <FrameCanvas subscribeFrames={subscribeFrames} acknowledgeFrame={acknowledgeFrame} />

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
