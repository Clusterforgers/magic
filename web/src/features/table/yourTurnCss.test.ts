// Goal: the "your turn" glow keeps its static ring, edge and shadow for everyone, declares its pulse only inside `prefers-reduced-motion: no-preference`, pulses for 6 s at most and then rests at the shadow it started from, so a person who asks for less motion never gets an animation from it and nobody gets one that never stops.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { blockAfter } from '@/test-utils/cssBlocks'

// Read as text rather than imported, which would hand back what Tailwind builds from it.
const css = readFileSync(join(import.meta.dirname, '../../index.css'), 'utf8')

const NO_PREFERENCE = '@media (prefers-reduced-motion: no-preference)'
/** The longest the glow may pulse, in seconds (docs/ui-design.md, Turn and focus). */
const MOST_SECONDS = 6

const squash = (text: string): string => text.replace(/\s+/g, ' ').trim()

describe('the your-turn cue stylesheet', () => {
  const cue = blockAfter(css, '@utility your-turn-cue')
  const motion = blockAfter(cue, NO_PREFERENCE)
  const still = cue.replace(motion, '')
  const shadowOf = (block: string): string => squash(/box-shadow:([^;]+);/.exec(block)?.[1] ?? '')

  it('pulses only where motion is welcome', () => {
    expect(motion).toMatch(/animation:\s*your-turn-glow/)
    expect(still).not.toMatch(/animation/)
  })

  it('keeps a ring and a glow without any motion', () => {
    expect(still).toMatch(/box-shadow:[^;]*var\(--turn-you-edge\)[^;]*color-mix\([^;]*var\(--turn-you-edge\)/)
  })

  it('has no other animation in the stylesheet that could slip out of the media query', () => {
    expect([...css.matchAll(/animation\s*:/g)]).toHaveLength(1)
  })

  it('pulses a finite number of times, 6 s at most, and then rests', () => {
    const match = /animation:\s*your-turn-glow\s+([\d.]+)(m?s)\s+\S+\s+(\S+);/.exec(motion)
    expect(match, motion).not.toBeNull()
    const seconds = Number(match?.[1]) / (match?.[2] === 'ms' ? 1000 : 1)
    const count = Number(match?.[3])
    expect(Number.isFinite(count), `${match?.[3]} runs`).toBe(true)
    expect(count).toBeGreaterThanOrEqual(1)
    expect(seconds * count).toBeLessThanOrEqual(MOST_SECONDS)
    expect(motion).not.toMatch(/infinite/)
  })

  it('pulses the same ring and edge that stand still, so the ring never jumps', () => {
    const frames = blockAfter(css, '@keyframes your-turn-glow')
    const shadows = [...frames.matchAll(/box-shadow:([^;]+);/g)].map((match) => squash(match[1] ?? ''))
    expect(shadows.length).toBeGreaterThanOrEqual(2)
    for (const shadow of shadows) {
      const [edge, ring] = shadow.split(/,\s*(?![^()]*\))/)
      expect(edge).toBe('inset 0 0 0 var(--cue-edge, 0px) var(--turn-you-edge)')
      expect(ring).toBe('0 0 0 2px var(--turn-you-edge)')
    }
    expect(shadowOf(still)).toContain('0 0 0 2px var(--turn-you-edge)')
  })

  it('starts and ends the pulse at the resting shadow, so it ends without a jump', () => {
    const frames = blockAfter(css, '@keyframes your-turn-glow')
    const rests = /0%,\s*100%\s*\{([^}]*)\}/.exec(frames)?.[1] ?? ''
    expect(shadowOf(rests)).toBe(shadowOf(still))
  })

  it('draws an edge inside only where an element asks for one with --cue-edge, so the strip keeps its look', () => {
    expect(shadowOf(still)).toMatch(/^inset 0 0 0 var\(--cue-edge, 0px\) var\(--turn-you-edge\), /)
    expect(css).not.toMatch(/--cue-edge\s*:/)
  })
})
