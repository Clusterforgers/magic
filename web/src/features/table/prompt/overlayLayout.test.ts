// Goal: the long-list overlay's panel is laid out from the room it has: cards 120 to 160 px wide on a desktop room (80 on a phone), blocks of cards per controlling seat side by side where the width allows, the panel never wider than its share of the room, shrinking only to fit a low window and scrolling once the cards cannot all stand in it.
import { describe, expect, it } from 'vitest'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import {
  BLOCK_GAP,
  CARD_GAP,
  PANEL_PAD,
  PANEL_SHARE,
  SCROLLBAR_PX,
  cardWidthFor,
  groupByController,
  overlayLayout,
} from './overlayLayout'
import type { OverlayRoom } from './overlayLayout'

const desktop = (width: number, height: number): OverlayRoom => ({ width, height, phone: false })
const phone = (width: number, height: number): OverlayRoom => ({ width, height, phone: true })

describe('cardWidthFor', () => {
  it.each([
    [desktop(600, 500), 120],
    [desktop(1200, 500), 120],
    [desktop(1400, 500), 140],
    [desktop(1580, 500), 158],
    [desktop(2300, 500), 160],
    [desktop(4000, 500), 160],
    [phone(374, 500), 80],
    [phone(900, 500), 80],
  ])('a room of %j draws cards %i px wide', (room, width) => {
    expect(cardWidthFor(room)).toBe(width)
  })
})

describe('overlayLayout', () => {
  it('stands thirteen cards of one player in two rows of seven where the room is large', () => {
    const layout = overlayLayout(desktop(2300, 1000), [13], 96)
    expect(layout).toMatchObject({ card: 160, columns: 7, scrolls: false })
    expect(layout.width).toBe(7 * 160 + 6 * CARD_GAP)
  })

  it('puts the blocks of four seats side by side in a wide room, with whole cards and no scroll', () => {
    const layout = overlayLayout(desktop(2300, 1000), [4, 4, 4, 4], 96)
    expect(layout).toMatchObject({ card: 160, columns: 2, scrolls: false })
    // Four blocks of two columns on one band, a gap between them.
    expect(layout.width).toBe(4 * (2 * 160 + CARD_GAP) + 3 * BLOCK_GAP)
  })

  it('shrinks cards toward the floor to fit a lower room before it scrolls', () => {
    const roomy = overlayLayout(desktop(1500, 760), [13], 96)
    const tight = overlayLayout(desktop(1500, 560), [13], 96)
    expect(roomy.scrolls).toBe(false)
    expect(tight.card).toBeLessThanOrEqual(roomy.card)
    expect(tight.card).toBeGreaterThanOrEqual(104)
  })

  it('keeps cards readable and scrolls where even the floor does not fit', () => {
    const layout = overlayLayout(desktop(1500, 560), [4, 4, 4, 4], 96)
    expect(layout.scrolls).toBe(true)
    expect(layout.card).toBeGreaterThanOrEqual(88)
    expect(layout.card).toBeLessThanOrEqual(120)
  })

  it('draws a low window one whole row of cards, smaller than the readable size', () => {
    const layout = overlayLayout(desktop(1000, 380), [4, 4, 4, 4], 96)
    expect(layout.scrolls).toBe(true)
    expect(layout.card).toBeGreaterThanOrEqual(88)
    expect(layout.card).toBeLessThan(120)
  })

  it('draws 80 px cards on a phone, in the whole width of the room', () => {
    const layout = overlayLayout(phone(374, 700), [13], 96)
    expect(layout.card).toBeLessThanOrEqual(80)
    expect(layout.card).toBeGreaterThanOrEqual(64)
    expect(layout.width + PANEL_PAD + SCROLLBAR_PX).toBeLessThanOrEqual(374)
  })

  it.each([
    [desktop(2300, 1000), [13]],
    [desktop(2300, 1000), [4, 4, 4, 4]],
    [desktop(1500, 560), [5, 4, 4]],
    [desktop(1000, 380), [40]],
    [desktop(800, 400), [2, 3, 8, 8, 8]],
    [phone(374, 500), [4, 4, 4, 4]],
    [phone(300, 500), [13]],
  ])('never takes more than its share of a %j room for %j', (room, sizes) => {
    const layout = overlayLayout(room, sizes, 96)
    const share = room.phone ? 1 : PANEL_SHARE
    expect(layout.width + PANEL_PAD + SCROLLBAR_PX).toBeLessThanOrEqual(Math.floor(room.width * share))
    expect(layout.columns).toBeGreaterThanOrEqual(1)
  })
})

/** A view with the one field the grouping reads. */
const viewOf = (id: ObjectId, controller: number): [ObjectId, ObjectView] => [
  id,
  { id, controller } as unknown as ObjectView,
]
const objects = new Map<ObjectId, ObjectView | undefined>([
  viewOf(1, 2),
  viewOf(2, 0),
  viewOf(3, 2),
  viewOf(4, 1),
  viewOf(5, 0),
])
const ALL = [1, 2, 3, 4, 5]

describe('groupByController', () => {
  it('groups by seat in seat order, each group in the order given', () => {
    expect(groupByController(ALL, ALL, objects)).toEqual([
      { seat: 0, cards: [2, 5] },
      { seat: 1, cards: [4] },
      { seat: 2, cards: [1, 3] },
    ])
  })

  it('leaves one player’s list in one group without a seat, so it has no labels', () => {
    const mine = [2, 5]
    expect(groupByController(mine, mine, objects)).toEqual([{ seat: null, cards: [2, 5] }])
  })

  it('keeps the seats of the whole list when a search narrows it to one seat', () => {
    expect(groupByController([3], ALL, objects)).toEqual([{ seat: 2, cards: [3] }])
  })

  it('does not group cards whose views are missing', () => {
    expect(groupByController([7, 8], [7, 8], new Map())).toEqual([{ seat: null, cards: [7, 8] }])
  })
})
