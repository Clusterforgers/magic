// Goal: the response clock's bar declares its animation only inside `prefers-reduced-motion: no-preference`, and the keyframes drain it from the part that was left to nothing, so a person who asks for less motion never gets an animation from it.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

// Read as text rather than imported, which would hand back what Tailwind builds from it.
const css = readFileSync(join(import.meta.dirname, '../../index.css'), 'utf8')

/** The text between the braces that open after `head`, nested braces included. */
function blockAfter(source: string, head: string): string {
  const start = source.indexOf(head)
  if (start < 0) throw new Error(`the stylesheet has no "${head}"`)
  const open = source.indexOf('{', start)
  let depth = 0
  for (let at = open; at < source.length; at += 1) {
    if (source[at] === '{') depth += 1
    if (source[at] === '}') depth -= 1
    if (depth === 0) return source.slice(open + 1, at)
  }
  throw new Error(`"${head}" never closes`)
}

const NO_PREFERENCE = '@media (prefers-reduced-motion: no-preference)'

describe('the response clock stylesheet', () => {
  const bar = blockAfter(css, '@utility response-clock-bar')
  const motion = blockAfter(bar, NO_PREFERENCE)
  const still = bar.replace(motion, '')

  it('moves only where motion is welcome', () => {
    expect(motion).toMatch(/animation:\s*response-clock-drain/)
    expect(still).not.toMatch(/animation/)
  })

  it('takes its time from the element, which sets animation-duration, so the shorthand carries none', () => {
    expect(motion).not.toMatch(/animation:[^;]*\d+m?s/)
    expect(motion).toMatch(/linear/)
    expect(motion).toMatch(/forwards/)
  })

  it('drains from the part that was left, to nothing, from its left edge', () => {
    const frames = blockAfter(css, '@keyframes response-clock-drain')
    expect(frames).toMatch(/from\s*\{[^}]*scaleX\(var\(--clock-from, 1\)\)/)
    expect(frames).toMatch(/to\s*\{[^}]*scaleX\(0\)/)
    expect(still).toMatch(/transform-origin:\s*left/)
  })
})
