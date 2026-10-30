// Goal: the arrow of an attachment is an accent line from the tokens (no colour of its own) that draws itself in over 450 ms only for a person who allows motion, while a person who asks for less gets it drawn still with a 100 ms fade and nothing that moves or dashes.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { blockAfter } from '@/test-utils/cssBlocks'

// Read as text rather than imported, which would hand back what the build makes of it.
const css = readFileSync(join(import.meta.dirname, 'attach.css'), 'utf8')

const NO_PREFERENCE = '@media (prefers-reduced-motion: no-preference)'
const REDUCE = '@media (prefers-reduced-motion: reduce)'
const motion = blockAfter(css, NO_PREFERENCE)
const reduced = blockAfter(css, REDUCE)
const plain = css.replace(motion, '').replace(reduced, '')

describe('the stylesheet of the attach arrow', () => {
  it('takes its colour from the accent token and writes no colour of its own', () => {
    expect(plain).toMatch(/\[data-kind='attach'\] \{\s*--line: var\(--primary\);/)
    expect(css).not.toMatch(/\b(?:oklch|rgba?|hsla?|color)\(|#[0-9a-fA-F]{3,8}\b/)
  })

  it('is thicker than an attack, so that it reads over the cards', () => {
    expect(plain).toMatch(/\.combat-line \{\s*stroke-width: 2\.75;/)
  })

  it('draws in over 450 ms inside the motion query and nowhere else', () => {
    expect(motion).toMatch(/animation-duration:\s*450ms/)
    expect(plain).not.toMatch(/animation/)
    expect(reduced).not.toMatch(/animation-duration|stroke-dash/)
  })

  it('fades in over 100 ms when motion is reduced, with no movement', () => {
    expect(reduced).toMatch(/animation:\s*attach-fade 100ms/)
    expect(reduced).not.toMatch(/translate|scale|rotate|transform/)
    expect(css.match(/@keyframes/g)).toHaveLength(1)
  })

  it('moves only the lines of the overlay and its ghost layer', () => {
    for (const block of [motion, reduced]) {
      for (const selector of block.matchAll(/^\s*([^{}@]+)\{/gm)) {
        const head = (selector[1] ?? '').trim()
        if (head === 'from' || head === 'to' || head.includes('keyframes')) continue
        expect(head).toMatch(/combat-overlay/)
      }
    }
  })
})
