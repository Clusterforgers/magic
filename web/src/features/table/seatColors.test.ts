// Goal: a seat's life figure and pod edge, painted in the seat's own hue, keep 4.5:1 and 3:1 on the plaque of both themes for every hue a seat can have.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { contrastRatio, parseOklch, toSrgb } from '@/test-utils/color'
import type { Mapping } from '@/test-utils/color'
import { blockAfter } from '@/test-utils/cssBlocks'
import { SEAT_EDGE, SEAT_LIFE } from './PodParts'
import { seatHue } from './podFigures'

const css = readFileSync(join(import.meta.dirname, '../../index.css'), 'utf8')
const surface = (block: string): string => /--surface:\s*([^;]+);/.exec(block)?.[1] ?? ''
const GROUND = { light: surface(blockAfter(css, '\n:root {')), dark: surface(blockAfter(css, '\n.dark {')) }

/** The light and dark colours of a class pair such as `text-[oklch(0.42_0.12_var(--seat-hue))] dark:text-[…]`, at a hue. */
function shades(classes: string, hue: number): { light: string; dark: string } {
  const found = [...classes.matchAll(/(dark:)?[a-z-]+-\[(oklch\([^\]]+\))\]/g)]
  const at = (dark: boolean): string =>
    (found.find((match) => (match[1] !== undefined) === dark)?.[2] ?? '')
      .replaceAll('_', ' ')
      .replace('var(--seat-hue)', String(hue))
  return { light: at(false), dark: at(true) }
}

const HUES = [...Array.from({ length: 360 }, (_, hue) => hue), ...[0, 1, 2, 3, 4, 5, 6, 7].map(seatHue)]
const MAPPINGS: Mapping[] = ['clip', 'chroma']
const ratio = (fg: string, bg: string, mapping: Mapping): number =>
  contrastRatio(toSrgb(parseOklch(fg), mapping), toSrgb(parseOklch(bg), mapping))

describe.each(['light', 'dark'] as const)('seat colours in the %s theme', (theme) => {
  it('reads the plaque the classes are painted on', () => {
    expect(GROUND[theme]).toMatch(/^oklch\(/)
  })

  it.each([
    ['life figure', SEAT_LIFE, 4.5],
    ['pod edge', SEAT_EDGE, 3],
  ] as const)('keeps the %s at %s:1 or more at every hue', (_, classes, min) => {
    for (const hue of HUES) {
      for (const mapping of MAPPINGS) {
        const colour = shades(classes, hue)[theme]
        expect(ratio(colour, GROUND[theme], mapping), `${colour} at ${hue}`).toBeGreaterThanOrEqual(min)
      }
    }
  })
})
