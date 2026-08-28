import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const styles = readFileSync(resolve(process.cwd(), 'src/styles.css'), 'utf8')
const html = readFileSync(resolve(process.cwd(), 'index.html'), 'utf8')
const manifest = JSON.parse(readFileSync(resolve(process.cwd(), 'public/manifest.webmanifest'), 'utf8')) as Record<
  string,
  unknown
>

describe('mobile controller shell', () => {
  it('keeps the PWA standalone and orientation-flexible', () => {
    expect(manifest).toMatchObject({ start_url: '/', scope: '/', display: 'standalone', orientation: 'any' })
  })

  it('suppresses controller gestures and exposes pressed feedback', () => {
    expect(styles).toContain('touch-action: none')
    expect(styles).toContain('overscroll-behavior: none')
    expect(styles).toContain('-webkit-touch-callout: none')
    expect(styles).toContain('[data-pressed="true"]')
    expect(html).toContain('viewport-fit=cover')
    expect(html).not.toContain('user-scalable=no')
  })

  it('contains explicit portrait and landscape layouts plus safe areas', () => {
    expect(styles).toContain('@media (orientation: portrait)')
    expect(styles).toContain('@media (orientation: landscape)')
    expect(styles).toContain('env(safe-area-inset-top)')
    expect(styles).toContain('env(safe-area-inset-bottom)')
  })

  it('reserves landscape space for the rotated action controls', () => {
    expect(styles).toContain('margin-inline: 0.75rem')
    expect(styles).toContain('width: clamp(3rem, 15dvh, 4rem)')
  })
})
