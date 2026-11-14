// Goal: a one-row hand lies in a gentle, symmetric arc (the middle card upright and highest, the ends turned outwards no more than 6° and no more than 8 px lower), a card stands straight as it rises, and a hand of several rows lies flat.
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { FAN_DROP_MOST, FAN_TURN_MOST, fanOf } from './HandFan'

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
