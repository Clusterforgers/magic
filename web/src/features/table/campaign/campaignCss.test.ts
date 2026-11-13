// Goal: the campaign stylesheet moves nothing for a person who asks for less motion (every transform and keyframe sits in the no-preference block, the reduced block only shortens the fade), changes only opacity outside it, never lets the phase banner take a pointer, fades in the turn banner's times and is part of the app's stylesheet.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { blockAfter, rulesIn } from '@/test-utils/cssBlocks'
import { BANNER_TIMES, BANNER_TIMES_REDUCED } from '../turn/timing'

const css = readFileSync(join(import.meta.dirname, 'campaign.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')
const app = readFileSync(join(import.meta.dirname, '../../../index.css'), 'utf8')

const NO_PREFERENCE = '@media (prefers-reduced-motion: no-preference)'
const REDUCE = '@media (prefers-reduced-motion: reduce)'
const motion = blockAfter(css, NO_PREFERENCE)
const reduced = blockAfter(css, REDUCE)
const outside = css.replace(motion, '').replace(reduced, '').replace(NO_PREFERENCE, '').replace(REDUCE, '')
const rulesOutside = rulesIn(outside).filter((rule) => rule.selector !== '')
const banner = rulesOutside.find((rule) => rule.selector === "[data-slot='phase-banner']")

const ms = (text: string, name: string): number => Number(new RegExp(`${name}:\\s*(\\d+)ms`).exec(text)?.[1])

describe('motion', () => {
  it('keeps every movement and keyframe inside the no-preference block', () => {
    expect(outside).not.toMatch(/animation|@keyframes|transform|translate|scale\(|rotate/)
    expect(motion).toMatch(/@keyframes phase-banner-in/)
    expect(motion).toMatch(/animation: phase-banner-in/)
  })

  it('transitions nothing but opacity outside it', () => {
    for (const rule of rulesOutside)
      for (const match of rule.body.matchAll(/transition:\s*([^;]+);/g))
        expect((match[1] ?? '').trim().split(/\s+/)[0]).toBe('opacity')
  })

  it('has nothing but the length of the fade in the reduced-motion block', () => {
    for (const rule of rulesIn(reduced))
      expect(rule.body.replace(/--phase-banner-(in|out):\s*\d+ms;/g, '').trim()).toBe('')
  })
})

describe('the phase banner', () => {
  it('never takes a pointer', () => {
    expect(banner?.body).toMatch(/pointer-events:\s*none/)
  })

  it('fades in the turn banner’s times', () => {
    expect(ms(banner?.body ?? '', '--phase-banner-in')).toBe(BANNER_TIMES.inMs)
    expect(ms(banner?.body ?? '', '--phase-banner-out')).toBe(BANNER_TIMES.outMs)
    expect(ms(reduced, '--phase-banner-in')).toBe(BANNER_TIMES_REDUCED.inMs)
    expect(ms(reduced, '--phase-banner-out')).toBe(BANNER_TIMES_REDUCED.outMs)
  })

  it('is part of the app’s stylesheet', () => {
    expect(app).toContain("@import './features/table/campaign/campaign.css';")
  })
})
