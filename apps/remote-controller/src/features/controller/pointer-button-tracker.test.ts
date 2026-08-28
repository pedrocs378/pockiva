import { describe, expect, it } from 'vitest'
import { PointerButtonTracker } from './pointer-button-tracker'

describe('PointerButtonTracker', () => {
  it('tracks simultaneous pointer ids independently', () => {
    const tracker = new PointerButtonTracker()
    expect(tracker.press(11, 'up')).toEqual({ button: 'up', pressed: true })
    expect(tracker.press(22, 'a')).toEqual({ button: 'a', pressed: true })
    expect(tracker.pressedButtons()).toEqual(['up', 'a'])
    expect(tracker.release(11)).toEqual({ button: 'up', pressed: false })
    expect(tracker.pressedButtons()).toEqual(['a'])
  })

  it('does not release a button until its final pointer ends', () => {
    const tracker = new PointerButtonTracker()
    expect(tracker.press(1, 'b')).toEqual({ button: 'b', pressed: true })
    expect(tracker.press(2, 'b')).toBeNull()
    expect(tracker.release(1)).toBeNull()
    expect(tracker.release(2)).toEqual({ button: 'b', pressed: false })
  })

  it('is idempotent for duplicate end events and clears all pointers atomically', () => {
    const tracker = new PointerButtonTracker()
    tracker.press(1, 'left')
    tracker.press(2, 'a')
    expect(tracker.release(99)).toBeNull()
    expect(tracker.clear()).toEqual(['left', 'a'])
    expect(tracker.clear()).toEqual([])
  })
})
