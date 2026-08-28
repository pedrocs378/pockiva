# PED-65 Remote Controller Joystick Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enlarge the landscape mobile controls and add a persistent, fixed, eight-direction virtual joystick that can replace the D-pad without changing protocol v1.

**Architecture:** Generalize the existing pointer tracker so one touch can own a set of digital buttons, then build joystick geometry and rendering as isolated controller-feature modules. The controller page owns the validated local preference and composes one directional surface at a time; the existing session continues to receive only digital button transitions.

**Tech Stack:** React 19, TypeScript, Pointer Events, Zod, browser localStorage, CSS orientation media queries, Vitest, Testing Library, Biome, pnpm.

---

## File map

- Modify `apps/remote-controller/src/features/controller/pointer-button-tracker.ts`: track a button set per pointer and calculate aggregate transitions.
- Modify `apps/remote-controller/src/features/controller/pointer-button-tracker.test.ts`: specify diagonal, overlap, release, and directional cleanup behavior.
- Modify `apps/remote-controller/src/features/controller/use-controller-input.ts`: expose set-per-pointer and selective button-release operations.
- Modify `apps/remote-controller/src/features/controller/use-controller-input.test.tsx`: verify transition delivery and selective cleanup.
- Create `apps/remote-controller/src/lib/storage.ts`: JSON boundary around injected browser storage.
- Create `apps/remote-controller/src/features/controller/directional-mode.ts`: validate and persist `d-pad | joystick`.
- Create `apps/remote-controller/src/features/controller/directional-mode.test.ts`: cover missing, valid, malformed, and unavailable storage.
- Create `apps/remote-controller/src/features/controller/joystick-direction.ts`: pure dead-zone, sector, and knob-clamping logic.
- Create `apps/remote-controller/src/features/controller/joystick-direction.test.ts`: cover cardinals, diagonals, boundaries, and dead zone.
- Create `apps/remote-controller/src/features/controller/VirtualJoystick.tsx`: own pointer capture and render the fixed base/knob.
- Create `apps/remote-controller/src/features/controller/VirtualJoystick.test.tsx`: cover pointer lifecycle and second-pointer rejection.
- Create `apps/remote-controller/src/features/controller/DirectionalControl.tsx`: render the selector and either D-pad or joystick.
- Create `apps/remote-controller/src/features/controller/DirectionalControl.test.tsx`: cover mode accessibility and forwarding.
- Modify `apps/remote-controller/src/pages/ControllerPage.tsx`: initialize and save the directional preference and release directions on mode changes.
- Modify `apps/remote-controller/src/pages/ControllerPage.test.tsx`: verify persistence, mode switching, diagonals, and multitouch.
- Modify `apps/remote-controller/src/styles.css`: add joystick styling and enlarge short landscape controls safely.
- Modify `apps/remote-controller/src/mobile-shell.test.ts`: lock responsive and safe-area guarantees.
- Create `docs/testing/ped-65-remote-joystick.md`: record automated and manual acceptance evidence.

### Task 1: Generalize pointer ownership to button sets

**Files:**
- Modify: `apps/remote-controller/src/features/controller/pointer-button-tracker.ts`
- Modify: `apps/remote-controller/src/features/controller/pointer-button-tracker.test.ts`

- [ ] **Step 1: Write failing tracker tests for diagonal ownership and selective release**

Replace single-transition expectations and add these cases:

```ts
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
```

- [ ] **Step 2: Run the tracker test and verify the new API is missing**

Run:

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test -- src/features/controller/pointer-button-tracker.test.ts
```

Expected: FAIL because `set` and `releaseButtons` do not exist and `release` does not return arrays.

- [ ] **Step 3: Implement aggregate transition calculation**

Use one set per pointer and compare aggregate state before and after every mutation:

```ts
import type { Button } from '@gameboy/protocol'
import { BUTTON_ORDER } from '@/constants/controller'

export type ButtonTransition = { button: Button; pressed: boolean }

export class PointerButtonTracker {
  private readonly pointers = new Map<number, ReadonlySet<Button>>()

  set(pointerId: number, buttons: readonly Button[]): ButtonTransition[] {
    const before = new Set(this.pressedButtons())
    const next = new Set(buttons)
    if (next.size === 0) this.pointers.delete(pointerId)
    else this.pointers.set(pointerId, next)
    return this.transitions(before, new Set(this.pressedButtons()))
  }

  press(pointerId: number, button: Button): ButtonTransition[] {
    return this.set(pointerId, [button])
  }

  release(pointerId: number): ButtonTransition[] {
    return this.set(pointerId, [])
  }

  releaseButtons(buttons: readonly Button[]): ButtonTransition[] {
    const removed = new Set(buttons)
    const before = new Set(this.pressedButtons())
    for (const [pointerId, owned] of this.pointers) {
      const retained = [...owned].filter((button) => !removed.has(button))
      if (retained.length === 0) this.pointers.delete(pointerId)
      else this.pointers.set(pointerId, new Set(retained))
    }
    return this.transitions(before, new Set(this.pressedButtons()))
  }

  clear(): Button[] {
    const buttons = this.pressedButtons()
    this.pointers.clear()
    return buttons
  }

  pressedButtons(): Button[] {
    const pressed = new Set([...this.pointers.values()].flatMap((buttons) => [...buttons]))
    return BUTTON_ORDER.filter((button) => pressed.has(button))
  }

  private transitions(before: ReadonlySet<Button>, after: ReadonlySet<Button>): ButtonTransition[] {
    return BUTTON_ORDER.flatMap((button) => {
      if (before.has(button) === after.has(button)) return []
      return [{ button, pressed: after.has(button) }]
    })
  }
}
```

- [ ] **Step 4: Run tracker tests**

Run the command from Step 2.

Expected: all pointer tracker tests PASS.

- [ ] **Step 5: Commit the tracker change**

```bash
rtk git add apps/remote-controller/src/features/controller/pointer-button-tracker.ts apps/remote-controller/src/features/controller/pointer-button-tracker.test.ts
rtk git commit -m "refactor(remote): track button sets per pointer"
```

### Task 2: Expose multi-button pointer input safely

**Files:**
- Modify: `apps/remote-controller/src/features/controller/use-controller-input.ts`
- Modify: `apps/remote-controller/src/features/controller/use-controller-input.test.tsx`

- [ ] **Step 1: Write failing hook tests for diagonal transitions and directional-only cleanup**

Add assertions that call the new API and inspect the mock server:

```ts
it('sends only changed transitions when a pointer moves between joystick sectors', async () => {
  const { session, server } = await connectedSession()
  const { result } = renderHook(() => useControllerInput(session))
  act(() => result.current.setPointerButtons(7, ['up', 'right']))
  act(() => result.current.setPointerButtons(7, ['right']))
  expect(server.receivedMessages.slice(-3)).toEqual([
    { type: 'button-down', button: 'up', sequence: 1 },
    { type: 'button-down', button: 'right', sequence: 2 },
    { type: 'button-up', button: 'up', sequence: 3 }
  ])
})

it('releases directions without releasing an action pointer', async () => {
  const { session, server } = await connectedSession()
  const { result } = renderHook(() => useControllerInput(session))
  act(() => {
    result.current.setPointerButtons(1, ['up', 'right'])
    result.current.pressPointer(2, 'a')
    result.current.releaseButtons(['up', 'down', 'left', 'right'])
  })
  expect(result.current.pressedButtons).toEqual(new Set(['a']))
  expect(server.receivedMessages.slice(-2)).toEqual([
    { type: 'button-up', button: 'up', sequence: 4 },
    { type: 'button-up', button: 'right', sequence: 5 }
  ])
})
```

- [ ] **Step 2: Run the hook tests and verify the missing methods fail**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test -- src/features/controller/use-controller-input.test.tsx
```

Expected: FAIL because `setPointerButtons` and `releaseButtons` are absent.

- [ ] **Step 3: Implement a shared transition dispatcher and public methods**

Extend the returned type and route every tracker mutation through one dispatcher:

```ts
export type ControllerInputState = {
  pressedButtons: ReadonlySet<Button>
  pressPointer: (pointerId: number, button: Button) => void
  setPointerButtons: (pointerId: number, buttons: readonly Button[]) => void
  releasePointer: (pointerId: number) => void
  releaseButtons: (buttons: readonly Button[]) => void
  releaseAll: () => void
}

const applyTransitions = useCallback(
  (transitions: readonly ButtonTransition[]) => {
    if (transitions.length === 0) return
    setPressedButtons(new Set(tracker.pressedButtons()))
    for (const transition of transitions) session?.setButton(transition.button, transition.pressed)
  },
  [session, tracker]
)

const setPointerButtons = useCallback(
  (pointerId: number, buttons: readonly Button[]) => applyTransitions(tracker.set(pointerId, buttons)),
  [applyTransitions, tracker]
)

const pressPointer = useCallback(
  (pointerId: number, button: Button) => applyTransitions(tracker.press(pointerId, button)),
  [applyTransitions, tracker]
)

const releasePointer = useCallback(
  (pointerId: number) => applyTransitions(tracker.release(pointerId)),
  [applyTransitions, tracker]
)

const releaseButtons = useCallback(
  (buttons: readonly Button[]) => applyTransitions(tracker.releaseButtons(buttons)),
  [applyTransitions, tracker]
)
```

Return all six operations, preserving the existing `releaseAll` visibility and page-hide behavior.

- [ ] **Step 4: Run hook, button, and page tests**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test -- src/features/controller/use-controller-input.test.tsx src/features/controller/ControllerButton.test.tsx src/pages/ControllerPage.test.tsx
```

Expected: all selected tests PASS.

- [ ] **Step 5: Commit the input API change**

```bash
rtk git add apps/remote-controller/src/features/controller/use-controller-input.ts apps/remote-controller/src/features/controller/use-controller-input.test.tsx
rtk git commit -m "feat(remote): support multi-button pointer input"
```

### Task 3: Add validated local directional-mode persistence

**Files:**
- Create: `apps/remote-controller/src/lib/storage.ts`
- Create: `apps/remote-controller/src/features/controller/directional-mode.ts`
- Create: `apps/remote-controller/src/features/controller/directional-mode.test.ts`

- [ ] **Step 1: Write failing persistence tests**

Create an injected in-memory backend and specify safe recovery:

```ts
import { describe, expect, it, vi } from 'vitest'
import { Storage } from '@/lib/storage'
import { DirectionalModeRepository } from './directional-mode'

const backend = (value: string | null = null) => ({
  getItem: vi.fn(() => value),
  setItem: vi.fn()
})

it('uses d-pad when no preference exists', () => {
  expect(new DirectionalModeRepository(new Storage(backend())).load()).toBe('d-pad')
})

it('restores a valid joystick preference', () => {
  expect(new DirectionalModeRepository(new Storage(backend('"joystick"'))).load()).toBe('joystick')
})

it.each(['not-json', '"unknown"', '{"mode":"joystick"}'])('repairs malformed value %s', (value) => {
  const raw = backend(value)
  expect(new DirectionalModeRepository(new Storage(raw)).load()).toBe('d-pad')
  expect(raw.setItem).toHaveBeenCalledWith('directionalModeV1', '"d-pad"')
})

it('keeps the current session usable when browser storage throws', () => {
  const raw = { getItem: vi.fn(() => { throw new Error('blocked') }), setItem: vi.fn(() => { throw new Error('blocked') }) }
  const repository = new DirectionalModeRepository(new Storage(raw))
  expect(repository.load()).toBe('d-pad')
  expect(() => repository.save('joystick')).not.toThrow()
})
```

- [ ] **Step 2: Run the persistence test and verify missing modules**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test -- src/features/controller/directional-mode.test.ts
```

Expected: FAIL because the storage and preference modules do not exist.

- [ ] **Step 3: Implement the JSON storage boundary**

Create `src/lib/storage.ts`:

```ts
export type StorageBackend = Pick<globalThis.Storage, 'getItem' | 'setItem'>

export class Storage {
  constructor(private readonly backend: StorageBackend) {}

  read(key: string): unknown {
    const value = this.backend.getItem(key)
    if (value === null) return null
    try {
      return JSON.parse(value)
    } catch {
      return value
    }
  }

  write(key: string, value: unknown): void {
    this.backend.setItem(key, JSON.stringify(value))
  }
}

export const browserStorage = new Storage({
  getItem: (key) => window.localStorage.getItem(key),
  setItem: (key, value) => window.localStorage.setItem(key, value)
})
```

- [ ] **Step 4: Implement the validated repository**

Create `src/features/controller/directional-mode.ts`:

```ts
import { z } from 'zod'
import { browserStorage, type Storage } from '@/lib/storage'

const SETTINGS_KEY = 'directionalModeV1'
const directionalModeSchema = z.enum(['d-pad', 'joystick'])

export type DirectionalMode = z.infer<typeof directionalModeSchema>
export const defaultDirectionalMode: DirectionalMode = 'd-pad'

export class DirectionalModeRepository {
  constructor(private readonly storage: Storage) {}

  load(): DirectionalMode {
    try {
      const value = this.storage.read(SETTINGS_KEY)
      if (value === null) return defaultDirectionalMode
      const parsed = directionalModeSchema.safeParse(value)
      if (parsed.success) return parsed.data
      this.save(defaultDirectionalMode)
      return defaultDirectionalMode
    } catch {
      return defaultDirectionalMode
    }
  }

  save(mode: DirectionalMode): void {
    const validated = directionalModeSchema.parse(mode)
    try {
      this.storage.write(SETTINGS_KEY, validated)
    } catch {
      // Storage availability is optional; the current session remains usable.
    }
  }
}

export const browserDirectionalModeRepository = new DirectionalModeRepository(browserStorage)
```

- [ ] **Step 5: Run tests and typecheck**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test -- src/features/controller/directional-mode.test.ts
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller typecheck
```

Expected: persistence tests and typecheck PASS.

- [ ] **Step 6: Commit preference persistence**

```bash
rtk git add apps/remote-controller/src/lib/storage.ts apps/remote-controller/src/features/controller/directional-mode.ts apps/remote-controller/src/features/controller/directional-mode.test.ts
rtk git commit -m "feat(remote): persist directional control mode"
```

### Task 4: Implement deterministic joystick geometry

**Files:**
- Create: `apps/remote-controller/src/features/controller/joystick-direction.ts`
- Create: `apps/remote-controller/src/features/controller/joystick-direction.test.ts`

- [ ] **Step 1: Write failing vector-resolution tests**

```ts
import { describe, expect, it } from 'vitest'
import { resolveJoystickVector } from './joystick-direction'

describe('resolveJoystickVector', () => {
  it('returns no buttons inside the dead zone', () => {
    expect(resolveJoystickVector({ x: 10, y: 0 }, 100)).toMatchObject({ buttons: [] })
  })

  it.each([
    [{ x: 100, y: 0 }, ['right']],
    [{ x: 0, y: 100 }, ['down']],
    [{ x: -100, y: 0 }, ['left']],
    [{ x: 0, y: -100 }, ['up']],
    [{ x: 100, y: -100 }, ['up', 'right']],
    [{ x: 100, y: 100 }, ['down', 'right']],
    [{ x: -100, y: 100 }, ['down', 'left']],
    [{ x: -100, y: -100 }, ['up', 'left']]
  ] as const)('maps vector %o to %o', (vector, buttons) => {
    expect(resolveJoystickVector(vector, 100).buttons).toEqual(buttons)
  })

  it('clamps the rendered knob to the maximum travel', () => {
    expect(resolveJoystickVector({ x: 300, y: 400 }, 100).knob).toEqual({ x: 60, y: 80 })
  })

  it('uses deterministic sectors on each side of the 22.5 degree boundary', () => {
    expect(resolveJoystickVector({ x: 100, y: -41 }, 100).buttons).toEqual(['right'])
    expect(resolveJoystickVector({ x: 100, y: -42 }, 100).buttons).toEqual(['up', 'right'])
  })
})
```

- [ ] **Step 2: Run the geometry test and verify the module is missing**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test -- src/features/controller/joystick-direction.test.ts
```

Expected: FAIL because `joystick-direction.ts` does not exist.

- [ ] **Step 3: Implement dead zone, sector mapping, and clamping**

```ts
import type { Button } from '@gameboy/protocol'

export type JoystickVector = { x: number; y: number }
export type JoystickResolution = { buttons: readonly Button[]; knob: JoystickVector }

const DEAD_ZONE_RATIO = 0.24

const sectorButtons = [
  ['right'],
  ['down', 'right'],
  ['down'],
  ['down', 'left'],
  ['left'],
  ['up', 'left'],
  ['up'],
  ['up', 'right']
] as const satisfies readonly (readonly Button[])[]

export const resolveJoystickVector = (vector: JoystickVector, maxDistance: number): JoystickResolution => {
  const distance = Math.hypot(vector.x, vector.y)
  const ratio = distance > maxDistance ? maxDistance / distance : 1
  const knob = { x: vector.x * ratio, y: vector.y * ratio }
  if (maxDistance <= 0 || distance < maxDistance * DEAD_ZONE_RATIO) return { buttons: [], knob }

  const degrees = ((Math.atan2(vector.y, vector.x) * 180) / Math.PI + 360) % 360
  const sector = Math.floor((degrees + 22.5) / 45) % 8
  return { buttons: sectorButtons[sector], knob }
}
```

- [ ] **Step 4: Run geometry tests and lint the module**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test -- src/features/controller/joystick-direction.test.ts
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller lint
```

Expected: geometry tests and remote-controller lint PASS.

- [ ] **Step 5: Commit joystick geometry**

```bash
rtk git add apps/remote-controller/src/features/controller/joystick-direction.ts apps/remote-controller/src/features/controller/joystick-direction.test.ts
rtk git commit -m "feat(remote): resolve virtual joystick directions"
```

### Task 5: Build the fixed virtual joystick component

**Files:**
- Create: `apps/remote-controller/src/features/controller/VirtualJoystick.tsx`
- Create: `apps/remote-controller/src/features/controller/VirtualJoystick.test.tsx`

- [ ] **Step 1: Write failing pointer lifecycle tests**

Render with spies, stub a 200-by-200 bounding rectangle, and cover move/release behavior:

```tsx
const setup = () => {
  const setPointerButtons = vi.fn()
  const releasePointer = vi.fn()
  render(<VirtualJoystick disabled={false} setPointerButtons={setPointerButtons} releasePointer={releasePointer} />)
  const joystick = screen.getByRole('group', { name: 'Virtual joystick' })
  joystick.getBoundingClientRect = () => ({
    x: 0, y: 0, top: 0, left: 0, right: 200, bottom: 200, width: 200, height: 200, toJSON: () => ({})
  })
  joystick.setPointerCapture = vi.fn()
  joystick.hasPointerCapture = vi.fn(() => true)
  joystick.releasePointerCapture = vi.fn()
  return { joystick, setPointerButtons, releasePointer }
}

it('captures one pointer and emits diagonal digital input', () => {
  const { joystick, setPointerButtons } = setup()
  fireEvent.pointerDown(joystick, { pointerId: 4, pointerType: 'touch', clientX: 100, clientY: 100 })
  fireEvent.pointerMove(joystick, { pointerId: 4, pointerType: 'touch', clientX: 180, clientY: 20 })
  expect(setPointerButtons).toHaveBeenLastCalledWith(4, ['up', 'right'])
  expect(joystick).toHaveAttribute('data-directions', 'up right')
})

it.each(['pointerUp', 'pointerCancel', 'lostPointerCapture'] as const)('releases on %s', (eventName) => {
  const { joystick, releasePointer } = setup()
  fireEvent.pointerDown(joystick, { pointerId: 4, pointerType: 'touch', clientX: 100, clientY: 100 })
  fireEvent[eventName](joystick, { pointerId: 4, pointerType: 'touch' })
  expect(releasePointer).toHaveBeenCalledWith(4)
})

it('ignores a second pointer while the first owns the joystick', () => {
  const { joystick, setPointerButtons } = setup()
  fireEvent.pointerDown(joystick, { pointerId: 4, pointerType: 'touch', clientX: 100, clientY: 100 })
  fireEvent.pointerDown(joystick, { pointerId: 5, pointerType: 'touch', clientX: 100, clientY: 100 })
  fireEvent.pointerMove(joystick, { pointerId: 5, pointerType: 'touch', clientX: 180, clientY: 100 })
  expect(setPointerButtons).not.toHaveBeenCalledWith(5, expect.anything())
})
```

- [ ] **Step 2: Run the component test and verify the component is missing**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test -- src/features/controller/VirtualJoystick.test.tsx
```

Expected: FAIL because `VirtualJoystick` does not exist.

- [ ] **Step 3: Implement fixed pointer capture and knob rendering**

Create a focused component with this public contract:

```tsx
import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import type { Button } from '@gameboy/protocol'
import { resolveJoystickVector, type JoystickVector } from './joystick-direction'

export type VirtualJoystickProps = {
  disabled: boolean
  setPointerButtons: (pointerId: number, buttons: readonly Button[]) => void
  releasePointer: (pointerId: number) => void
}

const centered: JoystickVector = { x: 0, y: 0 }

export const VirtualJoystick = ({ disabled, setPointerButtons, releasePointer }: VirtualJoystickProps) => {
  const activePointer = useRef<number | null>(null)
  const [knob, setKnob] = useState(centered)
  const [directions, setDirections] = useState<readonly Button[]>([])

  const update = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.pointerId !== activePointer.current) return
    const bounds = event.currentTarget.getBoundingClientRect()
    const maxDistance = Math.min(bounds.width, bounds.height) * 0.3
    const resolution = resolveJoystickVector(
      { x: event.clientX - (bounds.left + bounds.width / 2), y: event.clientY - (bounds.top + bounds.height / 2) },
      maxDistance
    )
    setKnob(resolution.knob)
    setDirections(resolution.buttons)
    setPointerButtons(event.pointerId, resolution.buttons)
  }

  const release = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.pointerId !== activePointer.current) return
    activePointer.current = null
    setKnob(centered)
    setDirections([])
    releasePointer(event.pointerId)
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
  }

  useEffect(() => {
    if (!disabled || activePointer.current === null) return
    releasePointer(activePointer.current)
    activePointer.current = null
    setKnob(centered)
    setDirections([])
  }, [disabled, releasePointer])

  return (
    <div
      className="virtual-joystick"
      role="group"
      aria-label="Virtual joystick"
      aria-disabled={disabled}
      data-directions={directions.join(' ')}
      onPointerDown={(event) => {
        if (disabled || activePointer.current !== null || (event.pointerType === 'mouse' && event.button !== 0)) return
        event.preventDefault()
        activePointer.current = event.pointerId
        event.currentTarget.setPointerCapture(event.pointerId)
        update(event)
      }}
      onPointerMove={update}
      onPointerUp={release}
      onPointerCancel={release}
      onLostPointerCapture={release}
      onContextMenu={(event) => event.preventDefault()}
    >
      <div className="joystick-knob" style={{ transform: `translate(${knob.x}px, ${knob.y}px)` }} />
      <span className="sr-only">{directions.length === 0 ? 'Centered' : directions.join(' ')}</span>
    </div>
  )
}
```

Add unmount cleanup that releases an active pointer exactly once without setting state during teardown:

```tsx
const releasePointerRef = useRef(releasePointer)
useEffect(() => {
  releasePointerRef.current = releasePointer
}, [releasePointer])

useEffect(
  () => () => {
    const pointerId = activePointer.current
    if (pointerId !== null) releasePointerRef.current(pointerId)
    activePointer.current = null
  }, []
)
```

- [ ] **Step 4: Run component and geometry tests**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test -- src/features/controller/VirtualJoystick.test.tsx src/features/controller/joystick-direction.test.ts
```

Expected: all selected tests PASS with no React act warnings.

- [ ] **Step 5: Commit the component**

```bash
rtk git add apps/remote-controller/src/features/controller/VirtualJoystick.tsx apps/remote-controller/src/features/controller/VirtualJoystick.test.tsx
rtk git commit -m "feat(remote): add fixed virtual joystick"
```

### Task 6: Compose selectable directional modes on the controller page

**Files:**
- Create: `apps/remote-controller/src/features/controller/DirectionalControl.tsx`
- Create: `apps/remote-controller/src/features/controller/DirectionalControl.test.tsx`
- Modify: `apps/remote-controller/src/pages/ControllerPage.tsx`
- Modify: `apps/remote-controller/src/pages/ControllerPage.test.tsx`

- [ ] **Step 1: Write failing directional-control tests**

Cover the native radio group and both rendered surfaces:

```tsx
it('renders an accessible mode selector and the selected directional surface', async () => {
  const user = userEvent.setup()
  const onModeChange = vi.fn()
  render(<DirectionalControl mode="d-pad" disabled={false} pressedButtons={new Set()} onModeChange={onModeChange} pressPointer={vi.fn()} setPointerButtons={vi.fn()} releasePointer={vi.fn()} />)
  expect(screen.getByRole('radiogroup', { name: 'Directional control' })).toBeVisible()
  expect(screen.getByRole('radio', { name: 'D-pad' })).toBeChecked()
  expect(screen.getByRole('button', { name: 'Up' })).toBeVisible()
  await user.click(screen.getByRole('radio', { name: 'Joystick' }))
  expect(onModeChange).toHaveBeenCalledWith('joystick')
})

it('renders the fixed joystick in joystick mode', () => {
  render(<DirectionalControl mode="joystick" disabled={false} pressedButtons={new Set()} onModeChange={vi.fn()} pressPointer={vi.fn()} setPointerButtons={vi.fn()} releasePointer={vi.fn()} />)
  expect(screen.getByRole('group', { name: 'Virtual joystick' })).toBeVisible()
  expect(screen.queryByRole('button', { name: 'Up' })).not.toBeInTheDocument()
})
```

- [ ] **Step 2: Implement `DirectionalControl`**

Use a fieldset containing native radio inputs and conditionally render the existing D-pad map or `VirtualJoystick`. The public props are:

```ts
export type DirectionalControlProps = {
  mode: DirectionalMode
  disabled: boolean
  pressedButtons: ReadonlySet<Button>
  onModeChange: (mode: DirectionalMode) => void
  pressPointer: (pointerId: number, button: Button) => void
  setPointerButtons: (pointerId: number, buttons: readonly Button[]) => void
  releasePointer: (pointerId: number) => void
}
```

Implement the selector and surface with this structure, reusing `ControllerButton`, `D_PAD_BUTTONS`, and `BUTTON_LABELS` unchanged:

```tsx
<div className="directional-control">
  <fieldset className="direction-mode-selector">
    <legend className="sr-only">Directional control</legend>
    <div role="radiogroup" aria-label="Directional control">
      {(['d-pad', 'joystick'] as const).map((option) => (
        <label key={option}>
          <input
            type="radio"
            name="directional-mode"
            value={option}
            checked={mode === option}
            onChange={() => onModeChange(option)}
          />
          <span>{option === 'd-pad' ? 'D-pad' : 'Joystick'}</span>
        </label>
      ))}
    </div>
  </fieldset>
  {mode === 'd-pad' ? (
    <fieldset className="d-pad">
      <legend className="sr-only">Directional controls</legend>
      {D_PAD_BUTTONS.map((button) => (
        <ControllerButton
          key={button}
          button={button}
          label={BUTTON_LABELS[button]}
          className={`control-button direction ${button}`}
          pressed={pressedButtons.has(button)}
          disabled={disabled}
          onPress={pressPointer}
          onRelease={releasePointer}
        />
      ))}
    </fieldset>
  ) : (
    <VirtualJoystick disabled={disabled} setPointerButtons={setPointerButtons} releasePointer={releasePointer} />
  )}
</div>
```

- [ ] **Step 3: Write failing page tests for persistence and safe mode switching**

Inject a `DirectionalModeRepository` through `ControllerPageProps`. Add a memory backend and assert:

```tsx
it('restores joystick mode and persists a switch back to d-pad', async () => {
  const user = userEvent.setup()
  const raw = { getItem: vi.fn(() => '"joystick"'), setItem: vi.fn() }
  render(<ControllerPage pairingUrl={pairedUrl} transport={server.createTransport()} directionalModeRepository={new DirectionalModeRepository(new Storage(raw))} />)
  expect(await screen.findByRole('group', { name: 'Virtual joystick' })).toBeVisible()
  await user.click(screen.getByRole('radio', { name: 'D-pad' }))
  expect(raw.setItem).toHaveBeenCalledWith('directionalModeV1', '"d-pad"')
  expect(screen.getByRole('button', { name: 'Up' })).toBeVisible()
})
```

Add a connected-session test that holds A and a joystick diagonal, changes to D-pad, and expects only `up` and `right` button-up messages while A remains pressed.

- [ ] **Step 4: Integrate mode state in `ControllerPage`**

Extend the props and initialize state synchronously:

```tsx
export type ControllerPageProps = {
  pairingUrl?: URL
  transport?: SessionTransport
  directionalModeRepository?: DirectionalModeRepository
}

export const ControllerPage = ({
  pairingUrl,
  transport,
  directionalModeRepository = browserDirectionalModeRepository
}: ControllerPageProps) => {
  const [directionalMode, setDirectionalMode] = useState(() => directionalModeRepository.load())
  // existing session and input setup
  const changeDirectionalMode = (mode: DirectionalMode) => {
    input.releaseButtons(D_PAD_BUTTONS)
    directionalModeRepository.save(mode)
    setDirectionalMode(mode)
  }
```

Replace the inline D-pad fieldset with `DirectionalControl`. Add `className="controller-title"` to the existing title container so landscape CSS can compact it without relying on structural selectors.

- [ ] **Step 5: Run directional and page tests**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test -- src/features/controller/DirectionalControl.test.tsx src/pages/ControllerPage.test.tsx
```

Expected: selector, persistence, D-pad, joystick, and session integration tests PASS.

- [ ] **Step 6: Commit controller composition**

```bash
rtk git add apps/remote-controller/src/features/controller/DirectionalControl.tsx apps/remote-controller/src/features/controller/DirectionalControl.test.tsx apps/remote-controller/src/pages/ControllerPage.tsx apps/remote-controller/src/pages/ControllerPage.test.tsx
rtk git commit -m "feat(remote): make directional control configurable"
```

### Task 7: Enlarge landscape controls without clipping

**Files:**
- Modify: `apps/remote-controller/src/styles.css`
- Modify: `apps/remote-controller/src/mobile-shell.test.ts`

- [ ] **Step 1: Write failing responsive contract tests**

Replace the current compact landscape assertions with explicit tokens for the enlarged layout and joystick:

```ts
it('enlarges landscape gameplay controls and keeps short screens safe', () => {
  expect(styles).toContain('@media (orientation: landscape) and (max-height: 32rem)')
  expect(styles).toContain('width: min(42dvh, 13rem)')
  expect(styles).toContain('width: clamp(4rem, 20dvh, 5.5rem)')
  expect(styles).toContain('env(safe-area-inset-left)')
  expect(styles).toContain('env(safe-area-inset-right)')
  expect(styles).toContain('.controller-title')
})

it('styles a fixed bounded joystick with touch gestures disabled', () => {
  expect(styles).toContain('.virtual-joystick')
  expect(styles).toContain('.joystick-knob')
  expect(styles).toMatch(/\.virtual-joystick\s*\{[\s\S]*?touch-action:\s*none;/)
})
```

- [ ] **Step 2: Run the shell test and verify the new tokens fail**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test -- src/mobile-shell.test.ts
```

Expected: FAIL because the joystick and enlarged landscape rules are absent.

- [ ] **Step 3: Add selector and joystick styles**

Add focused classes for the selector and fixed joystick. React owns only the knob transform:

```css
.directional-control {
  display: grid;
  justify-items: center;
  gap: 0.65rem;
}

.direction-mode-selector div {
  display: flex;
  gap: 0.2rem;
  padding: 0.2rem;
  border-radius: 999px;
  background: var(--muted);
}

.direction-mode-selector label {
  position: relative;
}

.direction-mode-selector input {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
}

.direction-mode-selector span {
  display: block;
  min-height: 44px;
  padding: 0.7rem 0.9rem;
  border-radius: 999px;
  color: var(--muted-foreground);
  font-size: 0.75rem;
  font-weight: 700;
}

.direction-mode-selector input:checked + span {
  color: var(--primary-foreground);
  background: var(--primary);
}

.direction-mode-selector input:focus-visible + span {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.virtual-joystick {
  position: relative;
  width: min(42vw, 12rem);
  aspect-ratio: 1;
  border: 2px solid #52525b;
  border-radius: 50%;
  background: radial-gradient(circle, #3f3f46 0 54%, #27272a 55% 72%, #18181b 73%);
  touch-action: none;
  -webkit-tap-highlight-color: transparent;
}

.virtual-joystick[aria-disabled='true'] {
  opacity: 0.72;
}

.joystick-knob {
  position: absolute;
  top: calc(50% - 23%);
  left: calc(50% - 23%);
  width: 46%;
  aspect-ratio: 1;
  border-radius: 50%;
  background: linear-gradient(145deg, #71717a, #27272a);
  box-shadow: 0 0.35rem 0.75rem rgb(0 0 0 / 45%);
  pointer-events: none;
  will-change: transform;
}
```

- [ ] **Step 4: Replace compact landscape sizes with enlarged responsive limits**

Within the existing landscape media query:

```css
.controller-title p,
.session-help,
footer {
  display: none;
}

.d-pad,
.virtual-joystick,
.action-controls {
  width: min(42dvh, 13rem);
}

.action {
  width: clamp(4rem, 20dvh, 5.5rem);
}
```

Keep Start/Select centered, preserve the safe-area padding already present, and add a nested media query for landscape viewports below 22rem height that reduces gaps/header copy before reducing gameplay controls. Do not exceed available height or introduce scrolling.

- [ ] **Step 5: Run shell, page, and full remote tests**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller test
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller lint
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller typecheck
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller build
```

Expected: every remote-controller test passes; lint, typecheck, and production build exit zero.

- [ ] **Step 6: Inspect portrait and landscape layouts visually**

Run:

```bash
rtk /Users/pedro/.local/bin/mise exec -- pnpm --filter @gameboy/remote-controller dev --host 127.0.0.1
```

Inspect at 390 by 844 portrait, 844 by 390 landscape, and 667 by 375 short landscape. Verify no overlap, clipping, or loss of safe-area padding in both D-pad and joystick modes.

- [ ] **Step 7: Commit responsive styling**

```bash
rtk git add apps/remote-controller/src/styles.css apps/remote-controller/src/mobile-shell.test.ts
rtk git commit -m "feat(remote): enlarge landscape touch controls"
```

### Task 8: Record evidence and run repository gates

**Files:**
- Create: `docs/testing/ped-65-remote-joystick.md`

- [ ] **Step 1: Create the PED-65 evidence document**

Record the implemented behavior, automated command results, browser viewport inspection, and a checklist for the real-phone acceptance. Explicitly state whether real-phone validation is complete or pending; do not mark it complete without user confirmation.

- [ ] **Step 2: Run all required JavaScript and Rust gates**

```bash
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm lint
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm typecheck
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm test
CI=true rtk /Users/pedro/.local/bin/mise exec -- pnpm build
rtk cargo fmt --all --check
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk cargo test --workspace --all-features
rtk cargo test -p gb-core --no-default-features
```

Expected: all commands exit zero. Record exact test counts in the evidence document.

- [ ] **Step 3: Review the final diff against PED-65**

Run:

```bash
rtk git diff --check
rtk git status --short
rtk git diff --stat 237a0a5...HEAD
```

Confirm protocol v1, desktop code, and `gb-core` have no feature changes. Confirm `package.json` and the pre-existing PED-39 plan remain untouched.

- [ ] **Step 4: Commit evidence**

```bash
rtk git add docs/testing/ped-65-remote-joystick.md
rtk git commit -m "docs: record PED-65 remote joystick evidence"
```

- [ ] **Step 5: Update Linear truthfully**

Post commit SHAs and automated/browser evidence to PED-65. Keep the issue `In Progress` until a real phone validates landscape sizing, eight directions, A/B multitouch, rotation, reconnect, and persisted mode. Move it to `Done` only after that confirmation.
