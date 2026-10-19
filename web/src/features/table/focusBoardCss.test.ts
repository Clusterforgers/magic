// Goal: the stylesheet of the focus board's cross-fade declares both animations (the board that arrives and the one that leaves) only inside `prefers-reduced-motion: no-preference`, for the 150 ms the code keeps the leaving board, stretched by the dev route's slow-down, changing opacity and nothing else, and is part of the app's stylesheet.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { blockAfter, rulesIn } from '@/test-utils/cssBlocks'
import { STYLESHEET } from '@/test-utils/themeTokens'
import { FOCUS_FADE_MS } from './useBoardFade'

// Read as text rather than imported, which would hand back what the build makes of it.
const css = readFileSync(join(import.meta.dirname, 'focusBoard.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')

const NO_PREFERENCE = '@media (prefers-reduced-motion: no-preference)'
const motion = blockAfter(css, NO_PREFERENCE)
const outside = css.replace(motion, '').replace(NO_PREFERENCE, '')

describe('the cross-fade stylesheet', () => {
  it('declares nothing outside the no-preference block, so reduced motion gets no animation from it', () => {
    expect(outside).not.toMatch(/animation|@keyframes|transition|opacity/)
    expect(outside.replace(/[{}\s]/g, '')).toBe('')
    expect(motion).toMatch(/animation:/)
    expect(css).not.toMatch(/prefers-reduced-motion:\s*reduce/)
  })

  it('animates the board that arrives and the board that leaves, each for the time the code keeps it', () => {
    const rules = rulesIn(motion)
    expect(rules.map((rule) => rule.selector)).toEqual([
      "[data-slot='focus-layer'][data-crossfade='in']",
      "[data-slot='focus-layer'][data-crossfade='out']",
    ])
    for (const rule of rules) {
      const match =
        /animation:\s*focus-board-(in|out)\s+calc\((\d+)ms \* var\(--fx-slow, 1\)\)\s+(.+?)\s+both;/.exec(
          rule.body,
        )
      expect(match, rule.body).not.toBeNull()
      expect(Number(match?.[2])).toBe(FOCUS_FADE_MS)
      expect(match?.[3]).toBe('cubic-bezier(0.2, 0, 0, 1)')
      expect(rule.selector).toContain(`data-crossfade='${match?.[1]}'`)
    }
  })

  it('fades in from nothing and out to nothing, and changes no other property', () => {
    const frames = [
      ...motion.matchAll(/@keyframes\s+(focus-board-(?:in|out))\s*\{\s*(from|to)\s*\{([^}]*)\}\s*\}/g),
    ]
    expect(frames.map((frame) => [frame[1], frame[2]])).toEqual([
      ['focus-board-in', 'from'],
      ['focus-board-out', 'to'],
    ])
    for (const frame of frames) expect(frame[3]?.replace(/\s+/g, '')).toBe('opacity:0;')
    expect(css).not.toMatch(/transform|translate|scale|rotate|filter|blur/)
  })

  it('holds the time to 150 ms, the cross-fade of the design', () => {
    expect(FOCUS_FADE_MS).toBe(150)
  })
})

describe('the app’s stylesheet', () => {
  it('imports the cross-fade', () => {
    expect(STYLESHEET).toContain("@import './features/table/focusBoard.css';")
  })

  it('has no animation of its own beyond the your-turn glow, which the your-turn test holds', () => {
    expect([...STYLESHEET.matchAll(/animation\s*:/g)]).toHaveLength(1)
  })
})
