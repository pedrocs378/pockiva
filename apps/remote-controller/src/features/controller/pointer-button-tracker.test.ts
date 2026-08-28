import { describe, expect, it } from 'vitest'
import { PointerButtonTracker } from './pointer-button-tracker'

describe('PointerButtonTracker', () => {
  it('tracks simultaneous pointer ids independently', () => {
    const tracker = new PointerButtonTracker()
    expect(tracker.press(11, 'up')).toEqual([{ button: 'up', pressed: true }])
    expect(tracker.press(22, 'a')).toEqual([{ button: 'a', pressed: true }])
    expect(tracker.pressedButtons()).toEqual(['up', 'a'])
    expect(tracker.release(11)).toEqual([{ button: 'up', pressed: false }])
    expect(tracker.pressedButtons()).toEqual(['a'])
  })

  it('does not release a button until its final pointer ends', () => {
    const tracker = new PointerButtonTracker()
    expect(tracker.press(1, 'b')).toEqual([{ button: 'b', pressed: true }])
    expect(tracker.press(2, 'b')).toEqual([])
    expect(tracker.release(1)).toEqual([])
    expect(tracker.release(2)).toEqual([{ button: 'b', pressed: false }])
  })

  it('is idempotent for duplicate end events and clears all pointers atomically', () => {
    const tracker = new PointerButtonTracker()
    tracker.press(1, 'left')
    tracker.press(2, 'a')
    expect(tracker.release(99)).toEqual([])
    expect(tracker.clear()).toEqual(['left', 'a'])
    expect(tracker.clear()).toEqual([])
  })

  it('replaces one pointer button set with aggregate transitions', () => {
    const tracker = new PointerButtonTracker()
    expect(tracker.set(11, ['up', 'right'])).toEqual([
      { button: 'up', pressed: true },
      { button: 'right', pressed: true }
    ])
    expect(tracker.set(11, ['right'])).toEqual([{ button: 'up', pressed: false }])
    expect(tracker.release(11)).toEqual([{ button: 'right', pressed: false }])
  })

  it('keeps overlapping buttons until their final pointer releases them', () => {
    const tracker = new PointerButtonTracker()
    tracker.set(1, ['up', 'right'])
    expect(tracker.set(2, ['right', 'a'])).toEqual([{ button: 'a', pressed: true }])
    expect(tracker.release(1)).toEqual([{ button: 'up', pressed: false }])
    expect(tracker.release(2)).toEqual([
      { button: 'right', pressed: false },
      { button: 'a', pressed: false }
    ])
  })

  it('removes only requested buttons from every pointer', () => {
    const tracker = new PointerButtonTracker()
    tracker.set(1, ['up', 'right'])
    tracker.set(2, ['a'])
    expect(tracker.releaseButtons(['up', 'down', 'left', 'right'])).toEqual([
      { button: 'up', pressed: false },
      { button: 'right', pressed: false }
    ])
    expect(tracker.pressedButtons()).toEqual(['a'])
  })
})
