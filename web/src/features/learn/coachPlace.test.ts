// Goal: the coach card is placed inside the window and never over a lit box or a kept-clear one (the band, the primary button, an open sheet, the preview) wherever a free place exists, at the three layouts the course is tested at; a phone docks it as a bar away from the lit box and folds it when nothing else fits.
import { describe, expect, it } from 'vitest'
import { EDGE, coachPlace } from './coachPlace'
import type { PlaceInput } from './coachPlace'
import type { Box } from './resolve'

const box = (x: number, y: number, width: number, height: number): Box => ({ x, y, width, height })
const overlaps = (a: Box, b: Box): boolean =>
  a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height

const DESKTOP = { width: 1920, height: 1080 }
const LAPTOP = { width: 1366, height: 768 }
const PHONE = { width: 390, height: 844 }

/** Desktop-like tables: the band across the middle, the primary button at the bottom right. */
const table = (win: { width: number; height: number }): Box[] => [
  box(110, win.height * 0.48, win.width - 500, 50),
  box(win.width - 360, win.height - 70, 140, 44),
]

const CASES: [string, PlaceInput][] = [
  [
    'a hand card at 1920',
    {
      window: DESKTOP,
      card: { width: 300, height: 180 },
      lit: [box(170, 980, 70, 98)],
      clear: table(DESKTOP),
    },
  ],
  [
    'the primary button at 1920',
    {
      window: DESKTOP,
      card: { width: 300, height: 160 },
      lit: [box(1560, 1010, 140, 44)],
      clear: table(DESKTOP),
    },
  ],
  [
    'a band answer at 1366',
    { window: LAPTOP, card: { width: 300, height: 170 }, lit: [box(700, 368, 80, 36)], clear: table(LAPTOP) },
  ],
  [
    'two lit boxes at 1366',
    {
      window: LAPTOP,
      card: { width: 300, height: 170 },
      lit: [box(20, 20, 200, 120), box(20, 600, 120, 60)],
      clear: table(LAPTOP),
    },
  ],
  [
    'the opponent pod at 1920',
    {
      window: DESKTOP,
      card: { width: 300, height: 200 },
      lit: [box(20, 60, 110, 300)],
      clear: table(DESKTOP),
    },
  ],
  [
    'nothing lit at 1366',
    { window: LAPTOP, card: { width: 300, height: 140 }, lit: [], clear: table(LAPTOP) },
  ],
]

describe('coachPlace on a wide window', () => {
  it.each(CASES)('keeps %s clear and stays inside', (_, input) => {
    const place = coachPlace(input)
    const card = box(place.x, place.y, input.card.width, input.card.height)
    expect(place.crowded).toBe(false)
    expect(card.x).toBeGreaterThanOrEqual(EDGE - 0.5)
    expect(card.y).toBeGreaterThanOrEqual(EDGE - 0.5)
    expect(card.x + card.width).toBeLessThanOrEqual(input.window.width - EDGE + 0.5)
    expect(card.y + card.height).toBeLessThanOrEqual(input.window.height - EDGE + 0.5)
    for (const each of [...input.lit, ...input.clear]) expect(overlaps(card, each)).toBe(false)
  })
})

describe('coachPlace on a phone', () => {
  it('floats between the docks when lit parts fill both ends', () => {
    const lit = [box(20, 60, 120, 170), box(20, 610, 360, 60)]
    const place = coachPlace({
      window: PHONE,
      card: { width: 374, height: 160 },
      lit,
      clear: [],
      phone: { top: 44, bottom: 700 },
    })
    expect(place.crowded).toBe(false)
    for (const each of lit) expect(overlaps(box(place.x, place.y, 374, 160), each)).toBe(false)
  })

  const phone = { top: 44, bottom: 700 }
  const band = box(8, 560, 374, 120)

  it('docks under the top bar while the lit box is in the lower half', () => {
    const place = coachPlace({
      window: PHONE,
      card: { width: 374, height: 120 },
      lit: [box(150, 600, 80, 40)],
      clear: [band],
      phone,
    })
    expect(place.y).toBeLessThan(PHONE.height / 2)
    expect(place.folded).toBe(false)
  })

  it('docks above the hand while the lit box is in the upper half and the band leaves room', () => {
    const place = coachPlace({
      window: PHONE,
      card: { width: 374, height: 120 },
      lit: [box(20, 80, 120, 160)],
      clear: [],
      phone,
    })
    expect(place.y).toBeGreaterThan(PHONE.height / 2)
  })

  it('never covers the band: it docks on top when the band fills the space above the hand', () => {
    const place = coachPlace({
      window: PHONE,
      card: { width: 374, height: 120 },
      lit: [box(20, 80, 120, 160)],
      clear: [band],
      phone,
    })
    expect(overlaps(box(place.x, place.y, 374, 120), band)).toBe(false)
  })

  it('folds to its title when neither dock has room', () => {
    const lit = [box(0, 50, 390, 200), box(0, 300, 390, 120), box(0, 560, 390, 140)]
    const place = coachPlace({ window: PHONE, card: { width: 374, height: 220 }, lit, clear: [band], phone })
    expect(place.folded).toBe(true)
  })
})
