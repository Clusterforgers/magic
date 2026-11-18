// Goal: a one-row hand lies in a gentle, symmetric arc (the middle card upright and highest, the ends turned outwards no more than 6° and no more than 8 px lower) raised so that at rest every card stays inside its slot below and at its outer side with its lowest corner on the floor, a card stands straight as it rises, and a hand of several rows lies flat.
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { FAN_DROP_MOST, FAN_TURN_MOST, fanOf, fanPlace, turnedReach } from './HandFan'

describe('fanOf', () => {
  it('stands a lone card straight', () => {
    expect(fanOf(0, 1)).toEqual({ turn: 0, drop: 0 })
  })

  it.each([2, 3, 7, 12, 24])('lays %i cards in a symmetric arc within its bounds', (count) => {
    const arc = Array.from({ length: count }, (_, index) => fanOf(index, count))
    for (const [index, place] of arc.entries()) {
      const mirror = arc[count - 1 - index]
      expect(place.turn).toBeCloseTo(-(mirror?.turn ?? 0), 5)
      expect(place.drop).toBeCloseTo(mirror?.drop ?? 0, 5)
      expect(Math.abs(place.turn)).toBeLessThanOrEqual(FAN_TURN_MOST)
      expect(place.drop).toBeGreaterThanOrEqual(0)
      expect(place.drop).toBeLessThanOrEqual(FAN_DROP_MOST)
    }
    expect(arc[0]?.turn).toBeLessThan(0)
    expect(arc.at(-1)?.turn).toBeGreaterThan(0)
  })
})

describe('fanPlace', () => {
  const boxes = [
    { width: 49, height: 68 },
    { width: 76, height: 106 },
    { width: 111, height: 155 },
    { width: 143, height: 200 },
  ]
  const cases = [1, 2, 3, 7, 12, 24].flatMap((count) => boxes.map((box) => [count, box] as const))

  it.each(cases)(
    'keeps %i turned cards of %o inside their slots at rest, the lowest corner on the floor',
    (count, box) => {
      const floors: number[] = []
      for (let index = 0; index < count; index++) {
        const place = fanPlace(index, count, box)
        const reach = turnedReach(place.turn, box)
        const foot = place.y + box.height + reach.y
        // Inside the slot's box, sideways and at the foot; only the lift's room above may be used.
        expect(foot).toBeLessThanOrEqual(box.height)
        // A turned card stays inside its slot on its outer side, towards the end of the hand it leans to.
        if (place.turn < 0) expect(place.x - reach.x).toBeGreaterThanOrEqual(-1e-9)
        if (place.turn > 0) expect(place.x + box.width + reach.x).toBeLessThanOrEqual(box.width + 1e-9)
        expect(-(place.y - reach.y)).toBeLessThan(box.height * 0.4)
        floors.push(foot)
      }
      // The arc is not lifted further than the rounding to a tenth of a pixel.
      expect(Math.max(...floors)).toBeGreaterThan(box.height - 0.15)
    },
  )

  it('reaches no further than the upright box when the card stands straight', () => {
    expect(turnedReach(0, { width: 100, height: 140 })).toEqual({ x: 0, y: 0 })
    expect(fanPlace(0, 1, { width: 100, height: 140 })).toEqual({ turn: 0, x: 0, y: 0 })
  })
})

describe('the hand at the table', () => {
  const backings = (container: HTMLElement): HTMLElement[] => [
    ...container.querySelectorAll<HTMLElement>('[data-region="hand"] [data-slot="hand-slot"] > div'),
  ]

  it('turns each card of a one-row hand and straightens it on hover and focus', () => {
    const { container } = renderTable({ seats: 2 })
    const cards = backings(container)
    expect(cards.length).toBeGreaterThan(1)
    expect(cards[0]?.style.getPropertyValue('--fan-turn')).toMatch(/^-\d/)
    for (const card of cards) expect(card).toHaveClass('rotate-(--fan-turn)', 'group-hover/slot:rotate-0')
  })
})
