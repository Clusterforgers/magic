// Goal: the "your turn" glow keeps its static ring and shadow for everyone but declares its pulse only inside `prefers-reduced-motion: no-preference`, so a person who asks for less motion never gets an animation from it.
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

describe('the your-turn cue stylesheet', () => {
  const cue = blockAfter(css, '@utility your-turn-cue')
  const motion = blockAfter(cue, NO_PREFERENCE)
  const still = cue.replace(motion, '')

  it('pulses only where motion is welcome', () => {
    expect(motion).toMatch(/animation:\s*your-turn-glow/)
    expect(still).not.toMatch(/animation/)
  })

  it('keeps a ring and a glow without any motion', () => {
    expect(still).toMatch(/box-shadow:[^;]*var\(--primary\)[^;]*var\(--your-turn\)/)
  })

  it('has no other animation in the stylesheet that could slip out of the media query', () => {
    expect([...css.matchAll(/animation\s*:/g)]).toHaveLength(1)
  })

  it('pulses the same ring that stands still, so the ring never jumps', () => {
    const frames = blockAfter(css, '@keyframes your-turn-glow')
    const shadows = [...frames.matchAll(/box-shadow:\s*([^;]*?),/g)].map((match) => match[1])
    expect(shadows.length).toBeGreaterThanOrEqual(2)
    for (const ring of shadows) expect(ring).toBe('0 0 0 2px var(--primary)')
    expect(still).toContain('0 0 0 2px var(--primary)')
  })
})
