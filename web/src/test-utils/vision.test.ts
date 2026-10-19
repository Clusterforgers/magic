// Goal: the colour-vision helpers behind the palette tests are right: the CIEDE2000 difference reproduces the published test pairs of Sharma, Wu and Dalal (2005), is zero for equal colours and the same both ways, CIELAB matches known colours, and each dichromacy simulation (Machado, Oliveira and Fernandes, 2009) leaves greys alone, stays on the screen's range, and pulls together the colours that people with that dichromacy confuse.
import { describe, expect, it } from 'vitest'
import type { Rgb } from './color'
import { colourDifference, DICHROMACIES, deltaE2000Lab, labOf, simulate } from './vision'
import type { Dichromacy, Lab } from './vision'

// Table I of Sharma, Wu and Dalal, "The CIEDE2000 color-difference formula": pair number, both samples, the published difference.
const PUBLISHED: ReadonlyArray<[number, Lab, Lab, number]> = [
  [1, [50, 2.6772, -79.7751], [50, 0, -82.7485], 2.0425],
  [2, [50, 3.1571, -77.2803], [50, 0, -82.7485], 2.8615],
  [3, [50, 2.8361, -74.02], [50, 0, -82.7485], 3.4412],
  [4, [50, -1.3802, -84.2814], [50, 0, -82.7485], 1.0],
  [7, [50, 0, 0], [50, -1, 2], 2.3669],
  [9, [50, 2.49, -0.001], [50, -2.49, 0.0009], 7.1792],
  [13, [50, -0.001, 2.49], [50, 0.0009, -2.49], 4.8045],
  [16, [50, 2.5, 0], [50, 0, -2.5], 4.3065],
  [17, [50, 2.5, 0], [73, 25, -18], 27.1492],
  [18, [50, 2.5, 0], [61, -5, 29], 22.8977],
  [19, [50, 2.5, 0], [56, -27, -3], 31.903],
  [20, [50, 2.5, 0], [58, 24, 15], 19.4535],
  [21, [50, 2.5, 0], [50, 3.1736, 0.5854], 1.0],
  [25, [60.2574, -34.0099, 36.2677], [60.4626, -34.1751, 39.4387], 1.2644],
  [26, [63.0109, -31.0961, -5.8663], [62.8187, -29.7946, -4.0864], 1.263],
  [28, [35.0831, -44.1164, 3.7933], [35.0232, -40.0716, 1.5901], 1.8645],
  [31, [90.8027, -2.0831, 1.441], [91.1528, -1.6435, 0.0447], 1.4441],
  [33, [6.7747, -0.2908, -2.4247], [5.8714, -0.0985, -2.2286], 0.6377],
  [34, [2.0776, 0.0795, -1.135], [0.9033, -0.0636, -0.5514], 0.9082],
]

describe('the CIEDE2000 difference', () => {
  it.each(PUBLISHED)('gives the published value for pair %i', (_pair, first, second, expected) => {
    expect(deltaE2000Lab(first, second)).toBeCloseTo(expected, 3)
  })

  it.each(PUBLISHED)('is the same both ways for pair %i', (_pair, first, second) => {
    expect(deltaE2000Lab(second, first)).toBeCloseTo(deltaE2000Lab(first, second), 10)
  })

  it('is zero for a colour and itself', () => {
    expect(colourDifference([0.2, 0.4, 0.6], [0.2, 0.4, 0.6])).toBe(0)
  })

  it('reads sRGB as CIELAB under D65: white, black and pure red', () => {
    const [white, black, red] = [labOf([1, 1, 1]), labOf([0, 0, 0]), labOf([1, 0, 0])]
    expect(white[0]).toBeCloseTo(100, 1)
    expect(Math.abs(white[1]) + Math.abs(white[2])).toBeLessThan(0.05)
    expect(black[0]).toBeCloseTo(0, 5)
    expect(red.map((value) => Math.round(value))).toEqual([53, 80, 67])
  })

  it('sees white against black as the largest step there is', () => {
    expect(colourDifference([1, 1, 1], [0, 0, 0])).toBeGreaterThan(95)
  })
})

describe.each(DICHROMACIES)('the %s simulation', (kind: Dichromacy) => {
  it.each([0, 0.1, 0.35, 0.5, 0.8, 1])('leaves the grey %d as it is', (level) => {
    const [r, g, b] = simulate([level, level, level], kind)
    for (const channel of [r, g, b]) expect(channel).toBeCloseTo(level, 4)
  })

  it('stays on the range of a screen for every colour of a coarse grid', () => {
    const steps = [0, 0.25, 0.5, 0.75, 1]
    for (const r of steps)
      for (const g of steps)
        for (const b of steps)
          for (const channel of simulate([r, g, b], kind)) {
            expect(channel).toBeGreaterThanOrEqual(0)
            expect(channel).toBeLessThanOrEqual(1)
          }
  })
})

describe('colours that people with a missing cone confuse', () => {
  const hex = (value: string): Rgb => [
    Number.parseInt(value.slice(1, 3), 16) / 255,
    Number.parseInt(value.slice(3, 5), 16) / 255,
    Number.parseInt(value.slice(5, 7), 16) / 255,
  ]

  it.each<[Dichromacy, string, Rgb, Rgb]>([
    ['protanopia', 'olive and vermillion', [0.5, 0.5, 0], hex('#D55E00')],
    ['deuteranopia', 'red and green', [1, 0, 0], [0, 0.6, 0]],
    ['tritanopia', 'orange and pink', hex('#E69F00'), [1, 0.6, 0.7]],
  ])('%s sees %s as near-twins that full vision tells apart plainly', (kind, _names, first, second) => {
    const full = colourDifference(first, second)
    const seen = colourDifference(simulate(first, kind), simulate(second, kind))
    expect(full).toBeGreaterThan(30)
    expect(seen).toBeLessThan(full / 4)
  })

  it('keeps blue and yellow apart for a person who lacks the long or medium cone', () => {
    for (const kind of ['protanopia', 'deuteranopia'] as const) {
      const seen = colourDifference(simulate(hex('#0072B2'), kind), simulate(hex('#F0E442'), kind))
      expect(seen, kind).toBeGreaterThan(40)
    }
  })
})
