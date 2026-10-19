// Goal: the turn-cue stylesheet moves nothing for a person who asks for less motion (every transform and keyframe sits in the no-preference block, the reduced block only shortens the banner's fade), changes only opacity outside it, never lets the banner take a pointer, takes the banner's fades from the same numbers as the timers, and is part of the app's stylesheet.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { blockAfter, rulesIn } from '@/test-utils/cssBlocks'
import { BANNER_TIMES, BANNER_TIMES_REDUCED } from './timing'

// Read as text rather than imported, which would hand back what Tailwind builds from it.
const css = readFileSync(join(import.meta.dirname, 'turn.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')
const app = readFileSync(join(import.meta.dirname, '../../../index.css'), 'utf8')

const NO_PREFERENCE = '@media (prefers-reduced-motion: no-preference)'
const REDUCE = '@media (prefers-reduced-motion: reduce)'
const motion = blockAfter(css, NO_PREFERENCE)
const reduced = blockAfter(css, REDUCE)
// What is left once both media blocks are cut out, with the empty braces they leave behind.
const outside = css.replace(motion, '').replace(reduced, '').replace(NO_PREFERENCE, '').replace(REDUCE, '')
const rulesOutside = rulesIn(outside).filter((rule) => rule.selector !== '')

const ms = (text: string, name: string): number => {
  const match = new RegExp(`${name}:\\s*(\\d+)ms`).exec(text)
  if (match === null) throw new Error(`no ${name} in the stylesheet`)
  return Number(match[1])
}

describe('motion', () => {
  it('keeps every movement and keyframe inside the no-preference block', () => {
    expect(outside).not.toMatch(/animation|@keyframes|transform|translate|scale|rotate/)
    expect(motion).toMatch(/animation:/)
    expect(motion).toMatch(/@keyframes/)
    expect(motion).toMatch(/transform/)
  })

  it('changes nothing but opacity outside it, which a fade needs and a still page can bear', () => {
    const touched: string[] = []
    for (const rule of rulesOutside) {
      for (const match of rule.body.matchAll(/transition:\s*([^;]+);/g)) {
        // Cut the easing and the variables, whose commas and spaces are not a property list.
        const list = (match[1] ?? '').replace(/cubic-bezier\([^)]*\)|var\([^)]*\)/g, '')
        touched.push(...list.split(',').map((part) => part.trim().split(/\s+/)[0] ?? ''))
      }
    }
    expect(touched.length).toBeGreaterThan(0)
    expect(new Set(touched)).toEqual(new Set(['opacity']))
  })

  it('has nothing but the length of the fade in the reduced-motion block', () => {
    expect(reduced).not.toMatch(/animation|@keyframes|transform|translate|scale|rotate|transition/)
    for (const rule of rulesIn(reduced))
      expect(rule.body.replace(/--turn-banner-(in|out):\s*\d+ms;/g, '').trim()).toBe('')
  })

  it('uses every keyframe it defines and defines every keyframe it uses', () => {
    const defined = [...motion.matchAll(/@keyframes\s+([\w-]+)/g)].map((match) => match[1])
    const used = [...motion.matchAll(/animation:\s*([\w-]+)/g)].map((match) => match[1])
    expect(defined.length).toBeGreaterThan(0)
    for (const name of used) expect(defined, `keyframes ${name}`).toContain(name)
    for (const name of defined) expect(used, `unused keyframes ${name}`).toContain(name)
  })

  it('only touches the banner and the ticker line', () => {
    for (const rule of [...rulesOutside, ...rulesIn(motion), ...rulesIn(reduced)]) {
      expect(rule.selector, rule.selector).toMatch(/^\[data-slot='turn-(banner|ticker-line)'\]/)
    }
  })
})

describe('the banner', () => {
  it('never takes a pointer, so a click goes through to what is under it', () => {
    const rule = rulesOutside.find((entry) => entry.selector === "[data-slot='turn-banner']")
    expect(rule?.body).toMatch(/pointer-events:\s*none/)
  })

  it('fades in and out in the times the timers use, and in the reduced times under reduced motion', () => {
    const base = rulesOutside.find((entry) => entry.selector === "[data-slot='turn-banner']")
    expect(ms(base?.body ?? '', '--turn-banner-in')).toBe(BANNER_TIMES.inMs)
    expect(ms(base?.body ?? '', '--turn-banner-out')).toBe(BANNER_TIMES.outMs)
    expect(ms(reduced, '--turn-banner-in')).toBe(BANNER_TIMES_REDUCED.inMs)
    expect(ms(reduced, '--turn-banner-out')).toBe(BANNER_TIMES_REDUCED.outMs)
  })

  it('is part of the app’s stylesheet', () => {
    expect(app).toContain("@import './features/table/turn/turn.css';")
  })
})
