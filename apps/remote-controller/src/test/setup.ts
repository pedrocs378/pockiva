import '@testing-library/jest-dom/vitest'
import { cleanup } from '@testing-library/react'
import { afterEach, vi } from 'vitest'

Object.defineProperties(HTMLElement.prototype, {
  setPointerCapture: { configurable: true, writable: true, value: vi.fn() },
  releasePointerCapture: { configurable: true, writable: true, value: vi.fn() },
  hasPointerCapture: { configurable: true, writable: true, value: vi.fn(() => true) }
})

afterEach(cleanup)
