/** Goal: the floating opening hand keeps its cards in one row inside the overlay's room, leaving the panel's sentence, answers and status their height, at every room a table off a phone gives it, from a narrow tablet to a wide desktop, for hands of up to seven cards, within the least and most card width. */
import { describe, expect, it } from 'vitest'
import { OPENING_CARD_LEAST_PX, OPENING_CARD_MOST_PX, openingCardPx } from './openingCard'

const GAP_PX = 8

describe('openingCardPx', () => {
  const ROOMS = [
    { width: 600, height: 420 },
    { width: 720, height: 380 },
    { width: 960, height: 480 },
    { width: 1300, height: 520 },
    { width: 1900, height: 700 },
  ]

  it.each(ROOMS.flatMap((room) => [1, 5, 7].map((count) => [room, count] as const)))(
    'fits one row in %j with %i cards',
    (room, count) => {
      const card = openingCardPx(room, count)
      expect(card).toBeGreaterThanOrEqual(OPENING_CARD_LEAST_PX)
      expect(card).toBeLessThanOrEqual(OPENING_CARD_MOST_PX)
      expect(count * card + (count - 1) * GAP_PX).toBeLessThanOrEqual(room.width * 0.9)
    },
  )

  it.each(ROOMS.flatMap((room) => [120, 160, 200].map((rest) => [room, rest] as const)))(
    'leaves the panel in %j the %i px of its sentence, answers and status',
    (room, rest) => {
      const card = openingCardPx(room, 7, rest)
      const tall = Math.round((card * 7) / 5)
      if (card > OPENING_CARD_LEAST_PX) expect(tall + rest).toBeLessThanOrEqual(room.height)
      expect(card).toBeLessThanOrEqual(openingCardPx(room, 7))
    },
  )

  it('grows the cards with the room up to the most', () => {
    const widths = ROOMS.map((room) => openingCardPx(room, 7))
    expect(widths).toEqual([...widths].sort((a, b) => a - b))
    expect(openingCardPx({ width: 4000, height: 2000 }, 7)).toBe(OPENING_CARD_MOST_PX)
  })
})
