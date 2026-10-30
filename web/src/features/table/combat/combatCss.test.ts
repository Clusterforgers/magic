// Goal: the combat stylesheet moves nothing for a person who asks for less motion: every animation, every keyframes rule and the stroke trick that lets a line draw itself in sit inside `prefers-reduced-motion: no-preference`; the lines stand fully drawn without them; and every outcome has a colour for light and for dark and a rule that gives a line that colour.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { blockAfter } from '@/test-utils/cssBlocks'
import { OUTCOMES } from './outcome'

// Read as text rather than imported, which would hand back what the build makes of it.
const css = readFileSync(join(import.meta.dirname, 'combat.css'), 'utf8')

const NO_PREFERENCE = '@media (prefers-reduced-motion: no-preference)'
const motion = blockAfter(css, NO_PREFERENCE)
const still = css.replace(motion, '')

describe('the combat stylesheet and motion', () => {
  it('has one media query for motion, and animates inside it', () => {
    expect(css.split(NO_PREFERENCE)).toHaveLength(2)
    expect(motion).toMatch(/animation:\s*combat-line-draw/)
    expect(motion).toMatch(/animation:\s*combat-line-fade/)
  })

  it('has no animation, keyframes or stroke trick outside it', () => {
    expect(still).not.toMatch(/animation/)
    expect(still).not.toMatch(/@keyframes/)
    expect(still).not.toMatch(/stroke-dashoffset/)
  })

  it('only dashes a line for the draw-in inside the media query: a line stands solid without motion', () => {
    expect([...motion.matchAll(/stroke-dasharray:\s*1;/g)]).toHaveLength(1)
    expect([...still.matchAll(/stroke-dasharray:\s*([^;]+);/g)].map((match) => match[1])).toEqual(['7 6'])
    expect(still).toMatch(/\[data-pending\] \.combat-line \{\s*stroke-dasharray: 7 6;/)
  })

  it('draws a line in over a short time, and the arrowhead after it', () => {
    const draw = /animation:\s*combat-line-draw\s+(\d+)ms/.exec(motion)
    const fade = /animation:\s*combat-line-fade\s+(\d+)ms/.exec(motion)
    expect(Number(draw?.[1])).toBeLessThanOrEqual(400)
    expect(Number(fade?.[1])).toBeLessThanOrEqual(400)
    expect(motion).toMatch(/\.combat-head \{\s*animation: combat-line-fade \d+ms ease-out \d+ms both/)
  })

  it('moves only its own lines, never the key that explains them', () => {
    for (const selector of motion.matchAll(/^\s*([^{}@]+)\{/gm)) {
      const head = selector[1] ?? ''
      if (head.includes('keyframes') || head.trim() === 'from' || head.trim() === 'to') continue
      expect(head).toMatch(/combat-overlay|combat-ghost|combat-stack/)
    }
  })
})

describe('the combat stylesheet and colour', () => {
  const light = blockAfter(css, ':root')
  const dark = blockAfter(css, '.dark')

  it.each(['attack', 'block', 'attacker-dies', 'blocker-dies', 'both-die', 'both-survive', 'damage'])(
    'defines the %s colour for light and for dark',
    (name) => {
      expect(light).toMatch(new RegExp(`--combat-${name}:`))
      expect(dark).toMatch(new RegExp(`--combat-${name}:`))
    },
  )

  it.each(OUTCOMES)('colours a line whose outcome is %s with the colour of that name', (outcome) => {
    expect(css).toMatch(
      new RegExp(
        `\\.combat-group\\[data-outcome='${outcome}'\\] \\{\\s*--line: var\\(--combat-${outcome}\\);`,
      ),
    )
  })

  it('gives each outcome a colour of its own', () => {
    const colours = OUTCOMES.map((outcome) => new RegExp(`--combat-${outcome}:\\s*([^;]+);`).exec(light)?.[1])
    expect(new Set(colours).size).toBe(OUTCOMES.length)
    expect(colours.every((colour) => colour !== undefined)).toBe(true)
  })

  it('keeps attacks muted and thin and blocks strong and thick', () => {
    const width = (head: string): number =>
      Number(
        new RegExp(`${head.replace(/[[\]()]/g, '\\$&')} \\{[^}]*stroke-width:\\s*([\\d.]+)`).exec(css)?.[1],
      )
    expect(width('.combat-line')).toBeLessThan(width(".combat-group[data-kind='block'] .combat-line"))
    expect(light).toMatch(/--combat-attack:\s*oklch\([^)]*\/\s*0\.\d+\)/)
  })
})

describe('the arrows of the stack in the stylesheet', () => {
  it('stand drawn in the accent or the destructive colour without motion and draw in over 250 ms with it', () => {
    expect(still).toMatch(/\.combat-group\[data-kind='target'\] \{\s*--line: var\(--destructive\)/)
    expect(still).toMatch(/\[data-kind='target'\]\[data-mine\] \{\s*--line: var\(--primary\)/)
    expect(motion).toMatch(/\.combat-stack[^{]*\{\s*animation-duration: calc\(250ms \* var\(--fx-slow, 1\)\)/)
  })
})
