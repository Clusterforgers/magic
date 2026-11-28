// Goal: a page tour's card sits on the free side of the part it explains, keeps its gap and stays inside every window, and its spotlight grows the part and is cut to the window.
import { describe, expect, it } from 'vitest'
import { GAP, MARGIN, placeCard, spotlight } from './tourPlace'
import type { Box } from './tourPlace'

const VIEW = { width: 1400, height: 900 }
const CARD = { width: 352, height: 180 }

const inside = (place: { left: number; top: number }, view = VIEW): boolean =>
  place.left >= MARGIN &&
  place.top >= MARGIN &&
  place.left + CARD.width <= view.width - MARGIN &&
  place.top + CARD.height <= view.height - MARGIN

describe('placeCard', () => {
  it.each<[string, Box | null, string]>([
    ['no target', null, 'centre'],
    ['the top bar', { left: 1300, top: 0, width: 40, height: 44 }, 'below'],
    ['the hand', { left: 0, top: 700, width: 1100, height: 200 }, 'above'],
    ['a tall side column', { left: 0, top: 0, width: 300, height: 900 }, 'right'],
    ['a tall column on the right', { left: 1100, top: 0, width: 300, height: 900 }, 'left'],
    ['the whole window', { left: 0, top: 0, width: 1400, height: 900 }, 'over'],
  ])('puts the card of %s on the %s side, inside the window', (_name, target, side) => {
    const place = placeCard(target, CARD, VIEW)
    expect(place.side).toBe(side)
    expect(inside(place)).toBe(true)
  })

  it('keeps the gap to a target it sits under', () => {
    const target = { left: 500, top: 100, width: 200, height: 60 }
    expect(placeCard(target, CARD, VIEW).top).toBe(target.top + target.height + GAP)
  })

  it.each([[{ width: 390, height: 844 }], [{ width: 844, height: 390 }]])(
    'stays inside a %o window',
    (view) => {
      const card = { width: view.width - 2 * MARGIN, height: 200 }
      const place = placeCard({ left: 0, top: 0, width: view.width, height: view.height }, card, view)
      expect(place.left).toBe(MARGIN)
      expect(place.top + card.height).toBeLessThanOrEqual(view.height - MARGIN)
    },
  )
})

describe('spotlight', () => {
  it('grows the target and cuts it to the window', () => {
    expect(spotlight({ left: 0, top: 10, width: 100, height: 50 }, VIEW)).toEqual({
      left: 0,
      top: 6,
      width: 104,
      height: 58,
    })
  })
})
