// Goal: the table-feedback stylesheet starts motion only where motion is welcome (every animation sits in the no-preference block, which holds all the keyframes, while reduced motion gets still outlines), applies only to elements that carry an effect attribute, takes every duration from the variables that mirror FX_MS and stretches it by the dev slow-down, and is part of the app's stylesheet.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { blockAfter, rulesIn } from '@/test-utils/cssBlocks'
import { FX_MS } from './model'
import type { FxKind } from './model'
import { MOVE_MS } from './moves'
import { EASE_OUT } from './travel'

// Read as text rather than imported, which would hand back what Tailwind builds from it.
const css = readFileSync(join(import.meta.dirname, 'fx.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')
const app = readFileSync(join(import.meta.dirname, '../../../index.css'), 'utf8')

const NO_PREFERENCE = '@media (prefers-reduced-motion: no-preference)'
const REDUCE = '@media (prefers-reduced-motion: reduce)'
const motion = blockAfter(css, NO_PREFERENCE)
const reduced = blockAfter(css, REDUCE)
const outside = css.replace(motion, '').replace(reduced, '')
const KINDS = Object.keys(FX_MS) as FxKind[]

describe('motion', () => {
  it('lives only inside the no-preference block', () => {
    expect(outside).not.toMatch(/animation|transition|@keyframes/)
    expect(motion).toMatch(/animation:/)
    expect(motion).toMatch(/@keyframes/)
  })

  it('is absent from the reduced-motion block, which only outlines', () => {
    expect(reduced).not.toMatch(/animation|transition|@keyframes|translate|scale|rotate|transform/)
    expect(reduced).toMatch(/outline:/)
  })

  it('uses every keyframe it defines and defines every keyframe it uses', () => {
    const defined = [...motion.matchAll(/@keyframes\s+([\w-]+)/g)].map((match) => match[1])
    const used = [...motion.matchAll(/animation:\s*([\w-]+)/g)].map((match) => match[1])
    expect(defined.length).toBeGreaterThan(0)
    for (const name of used) expect(defined, `keyframes ${name}`).toContain(name)
    for (const name of defined) expect(used, `unused keyframes ${name}`).toContain(name)
  })

  it('only ever touches elements that carry an effect attribute or class', () => {
    const rules = rulesIn(motion).filter((rule) => /animation/.test(rule.body))
    expect(rules.length).toBeGreaterThan(5)
    for (const rule of rules) expect(rule.selector, rule.selector).toMatch(/data-fx|data-ghost|\.fx-/)
    for (const rule of rulesIn(reduced)) expect(rule.selector, rule.selector).toMatch(/data-fx/)
  })
})

describe('durations', () => {
  it.each(KINDS)('mirrors FX_MS for %s', (kind) => {
    const value = new RegExp(`--fx-${kind}-ms:\\s*(\\d+)ms`).exec(css)?.[1]
    expect(Number(value)).toBe(FX_MS[kind])
  })

  it('is never a number written into an animation, always a variable stretched by --fx-slow', () => {
    const lines = [...motion.matchAll(/animation(?:-duration)?:[^;]*;/g)].map((match) => match[0])
    expect(lines.length).toBeGreaterThan(5)
    for (const line of lines) {
      expect(line, line).not.toMatch(/\d+m?s\b/)
      expect(line, line).toMatch(/var\(--fx-\w+-ms\)/)
      expect(line, line).toMatch(/var\(--fx-slow, 1\)/)
    }
  })

  it('keeps the dev slow-down out of everything but animation times', () => {
    const uses = [...css.matchAll(/--fx-slow/g)].length
    const inTimes = [...css.matchAll(/(?:animation|animation-duration):[^;]*--fx-slow[^;]*;/g)].length
    expect(uses).toBe(inTimes)
  })
})

describe('moves', () => {
  it.each(Object.entries(MOVE_MS))('mirrors MOVE_MS for %s', (name, ms) => {
    const value = new RegExp(`--fx-${name}-ms:\\s*(\\d+)ms`).exec(css)?.[1]
    expect(Number(value)).toBe(ms)
  })

  it("flies on a layer outside the layout that takes no pointer, and sizes an ability's chip by variables", () => {
    const layer = rulesIn(outside).find((rule) => rule.selector === '.fx-travel')
    expect(layer?.body).toMatch(/position:\s*fixed/)
    expect(layer?.body).toMatch(/pointer-events:\s*none/)
    const chip = rulesIn(outside).find((rule) => rule.selector.includes('.fx-flier[data-chip]'))
    expect(chip?.body).toMatch(/width:\s*var\(--fx-chip-w\)/)
  })

  it('eases as the app stylesheet does', () => {
    expect(app).toContain(`--ease-out: ${EASE_OUT};`)
  })
})

describe('where it is read', () => {
  it('is imported by the app stylesheet', () => {
    expect(app).toMatch(/@import\s+'\.\/features\/table\/fx\/fx\.css';/)
  })

  it('still leaves the your-turn pulse as the only animation of the app stylesheet itself', () => {
    expect([...app.matchAll(/animation\s*:/g)]).toHaveLength(1)
  })
})
