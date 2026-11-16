// Goal: the table-feedback stylesheet starts motion only where motion is welcome (every animation sits in the no-preference block, which holds all the keyframes, while reduced motion gets still outlines), applies only to elements that carry an effect attribute, takes every duration from the variables that mirror FX_MS and stretches it by the dev slow-down, repeats the move times of `MOVE_MS`, lays the layer of flying cards outside the layout without a pointer, moves pose to pose (an attacker winds up, lunges stretched and recoils, a hit squashes, lights and shakes, an entering card and a life total overshoot and settle, a dying card shrinks and tips over), and is part of the app’s stylesheet.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { blockAfter, rulesIn } from '@/test-utils/cssBlocks'
import { FX_MS, MARK_FX } from './model'
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
const KINDS = (Object.keys(FX_MS) as FxKind[]).filter(
  (kind) => !(MARK_FX as readonly string[]).includes(kind),
)

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

describe('feel (docs/ui-design.md, Feel)', () => {
  /** The declarations of one keyframe step of a named animation, such as `42%`. */
  const step = (name: string, at: string): string => {
    const frames = blockAfter(motion, `@keyframes ${name}`)
    const start = frames.search(new RegExp(`(^|\\s|,)${at}\\s*\\{`))
    if (start < 0) throw new Error(`${name} has no ${at}`)
    return blockAfter(frames.slice(start), at)
  }

  it('winds an attacker up away from its target, lunges it stretched and lets it recoil past its place', () => {
    expect(step('fx-lunge', '18%')).toMatch(/translate: 0 calc\(var\(--fx-dir, 1\) \* -0\.3rem\)/)
    expect(step('fx-lunge', '18%')).toMatch(/scale: 1\.03 0\.96/)
    expect(step('fx-lunge', '42%')).toMatch(/translate: 0 calc\(var\(--fx-dir, 1\) \* 1\.1rem\)/)
    expect(step('fx-lunge', '42%')).toMatch(/scale: 0\.97 1\.04/)
    expect(step('fx-lunge', '70%')).toMatch(/-0\.15rem/)
  })

  it('eases into the lunge and out of it', () => {
    expect(step('fx-lunge', '18%')).toMatch(/animation-timing-function: cubic-bezier\(0\.4, 0, 1, 1\)/)
    expect(step('fx-lunge', '42%')).toMatch(/animation-timing-function: cubic-bezier\(0, 0, 0\.2, 1\)/)
  })

  it('squashes and lights a hit card on impact, then shakes it 5 px', () => {
    const impact = step('fx-shake', '10%')
    expect(impact).toMatch(/scale: 0\.94/)
    expect(impact).toMatch(/translate: -5px 0/)
    expect(impact).toMatch(/filter: brightness\(1\.9\)/)
  })

  it('lets an entering permanent and a life total overshoot and settle', () => {
    expect(step('fx-enter', '60%')).toMatch(/scale: 1\.05/)
    expect(step('fx-enter', '82%')).toMatch(/scale: 0\.98/)
    expect(step('fx-pop', '20%')).toMatch(/scale: 1\.25/)
    expect(step('fx-pop', '45%')).toMatch(/scale: 0\.96/)
  })

  it('lets a dying card shrink and tip over as it greys', () => {
    expect(step('fx-ghost', '100%')).toMatch(/scale: 0\.85/)
    expect(step('fx-ghost', '100%')).toMatch(/rotate: -6deg/)
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
  it.each(MARK_FX)('holds a mark of %s for MOVE_MS.mark', (kind) => {
    expect(FX_MS[kind]).toBe(MOVE_MS.mark)
  })

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

  it('hides a flier until its leg starts, so a delayed leg never stands at its start', () => {
    const flier = rulesIn(outside).find((rule) => rule.selector === '.fx-flier')
    expect(flier?.body).toMatch(/opacity:\s*0/)
  })

  it('frames the viewer’s own spotlight with the plain border, the rival’s with the strong one', () => {
    const rules = rulesIn(outside)
    const caption = rules.find((rule) => rule.selector === '.fx-spotlight-caption')
    const own = rules.filter((rule) => rule.selector.startsWith('.fx-spotlight[data-own]'))
    expect(caption?.body).toContain('var(--border-strong)')
    expect(own).toHaveLength(2)
    for (const rule of own) {
      expect(rule.body, rule.selector).toContain('var(--border)')
      expect(rule.body, rule.selector).not.toContain('--border-strong')
      expect(rule.body, rule.selector).not.toMatch(/pointer-events:\s*auto/)
    }
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
