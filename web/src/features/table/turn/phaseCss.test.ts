// Goal: the phase stylesheet points the live theme tokens at the right set for each of the four phases and only under the table root, tints the table by at most 6 %, fades a change of theme in the design system's 200 ms inside the no-preference block only and turns the fade off under reduced motion, changes nothing but colours, and is part of the app's stylesheet.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { blockAfter, rulesIn } from '@/test-utils/cssBlocks'
import { ROOT, STYLESHEET } from '@/test-utils/themeTokens'
import { PHASES } from './phase'
import type { PhaseId } from './phase'

// Read as text rather than imported, which would hand back what Tailwind builds from it.
const css = readFileSync(join(import.meta.dirname, 'phase.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')

const NO_PREFERENCE = '@media (prefers-reduced-motion: no-preference)'
const REDUCE = '@media (prefers-reduced-motion: reduce)'
const motion = blockAfter(css, NO_PREFERENCE)
const reduced = blockAfter(css, REDUCE)
// What is left once both media blocks are cut out, with the empty braces they leave behind.
const outside = css.replace(motion, '').replace(reduced, '').replace(NO_PREFERENCE, '').replace(REDUCE, '')
const rulesOutside = rulesIn(outside).filter((rule) => rule.selector !== '')

const IDS = Object.keys(PHASES) as PhaseId[]
const ROOT_SLOT = "[data-slot='table-grid']"
const WASHED = `${ROOT_SLOT}[data-phase] > [data-slot='table-main']`
const FADING = `${ROOT_SLOT} [data-themed]`

const declared = (body: string): Map<string, string> =>
  new Map(
    [...body.matchAll(/(--[a-z-]+|[a-z-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1] ?? '', (m[2] ?? '').trim()]),
  )

describe('the themes', () => {
  it.each(IDS)('points the live tokens at the set of %s under the table root', (id) => {
    const rule = rulesOutside.find((entry) => entry.selector === `${ROOT_SLOT}[data-phase='${id}']`)
    expect(rule, `a rule for ${id}`).toBeDefined()
    expect([...declared(rule?.body ?? '')]).toEqual([
      ['--phase', `var(--phase-${id})`],
      ['--phase-soft', `var(--phase-${id}-soft)`],
      ['--on-phase', `var(--on-phase-${id})`],
    ])
  })

  it('has one rule for each phase and no other rule that sets a live token', () => {
    const setters = rulesOutside.filter((rule) => /--(phase|phase-soft|on-phase)\s*:/.test(rule.body))
    expect(setters.map((rule) => rule.selector).sort()).toEqual(
      IDS.map((id) => `${ROOT_SLOT}[data-phase='${id}']`).sort(),
    )
  })

  it('reaches only what lies under the table root, so no other page takes a theme', () => {
    for (const rule of [...rulesOutside, ...rulesIn(motion), ...rulesIn(reduced)]) {
      for (const selector of rule.selector.split(',')) {
        expect(selector.trim(), selector).toMatch(/^\[data-slot='table-grid'\]/)
      }
    }
  })

  it('leaves the live tokens neutral in the stylesheet of the app until a phase is named', () => {
    expect(ROOT.get('phase')).toBe('var(--border)')
    expect(ROOT.get('phase-soft')).toBe('var(--muted)')
    expect(ROOT.get('on-phase')).toBe('var(--foreground)')
  })
})

describe('the wash', () => {
  const rule = rulesOutside.find((entry) => entry.selector === WASHED)

  it('lies over the main area of a table that has a phase, and only there', () => {
    expect(rule).toBeDefined()
    expect(rulesOutside.filter((entry) => /background/.test(entry.body))).toHaveLength(1)
  })

  it('is the accent mixed into the table’s ground, at most 6 %', () => {
    const match = /background-color:\s*color-mix\(in srgb, var\(--phase\) ([\d.]+)%, var\(--table\)\)/.exec(
      rule?.body ?? '',
    )
    expect(match, rule?.body).not.toBeNull()
    expect(Number(match?.[1])).toBeGreaterThan(0)
    expect(Number(match?.[1])).toBeLessThanOrEqual(6)
  })
})

describe('the change between themes', () => {
  const transition = (body: string): string => /transition:([^;]+);/.exec(body)?.[1] ?? ''
  const rule = rulesIn(motion).find((entry) => entry.selector.includes('[data-themed]'))

  it('fades the table and the pieces that carry data-themed, and nothing else', () => {
    expect(rule).toBeDefined()
    expect(rule?.selector.split(',').map((selector) => selector.trim())).toEqual([WASHED, FADING])
    expect(rulesIn(motion)).toHaveLength(1)
  })

  it('takes the design system’s base time, 200 ms', () => {
    expect(ROOT.get('duration-base')).toBe('200ms')
    const parts = transition(rule?.body ?? '')
      .split(/,\s*(?=[a-z-]+\s)/)
      .map((part) => part.trim())
    expect(parts).toHaveLength(3)
    for (const part of parts)
      expect(part).toMatch(/^[a-z-]+ var\(--duration-base\) cubic-bezier\(0\.2, 0, 0, 1\)$/)
  })

  it('changes colours and nothing else, so no layout, size or movement', () => {
    const properties = transition(rule?.body ?? '')
      .split(/,\s*(?=[a-z-]+\s)/)
      .map((part) => part.trim().split(/\s+/)[0])
    expect(properties).toEqual(['background-color', 'border-color', 'color'])
    expect(css).not.toMatch(/animation|@keyframes|transform|translate|scale|rotate/)
  })

  it('sits inside the no-preference block alone: nothing outside it transitions', () => {
    expect(outside).not.toMatch(/transition/)
    expect(motion).toMatch(/transition/)
  })

  it('is switched off under reduced motion, for the same table and pieces, and does nothing else there', () => {
    const rules = rulesIn(reduced)
    expect(rules).toHaveLength(1)
    expect(rules[0]?.selector.split(',').map((selector) => selector.trim())).toEqual([WASHED, FADING])
    expect(rules[0]?.body.replace(/\s+/g, ' ').trim()).toBe('transition: none;')
  })
})

describe('the app’s stylesheet', () => {
  it('imports the phase themes', () => {
    expect(STYLESHEET).toContain("@import './features/table/turn/phase.css';")
  })

  it('has no animation of its own beyond the your-turn glow, which the your-turn test holds', () => {
    expect([...STYLESHEET.matchAll(/animation\s*:/g)]).toHaveLength(1)
  })
})
