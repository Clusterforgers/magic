// Goal: a fight against a boss opens in boss mode for any party of 1 to 6 heroes and 0 to 2 minions: the boss's side (the boss first) across the top, the other heroes together beside the viewer at the bottom, every seat drawn once; a phone tiles both sides; the menu can leave the mode; a seat of the boss's side or a table without a boss never gets it.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { withBoss } from './dev/devGame'
import {
  ALLY_STRIP_PX,
  ALLIES_MAX_PX,
  ALLIES_MIN_PX,
  ALLIES_OWN_MOST,
  alliesBandPx,
  arrangeSeats,
  planTable,
} from './layout'
import type { BossSides } from './layout'
import { bossSidesOf, parseBossSides } from './selectors'

/** The sides of a fight of `heroes` heroes from seat 0, the boss after them, then `minions` minions. */
function sides(heroes: number, minions: number): BossSides {
  const enemies = Array.from({ length: 1 + minions }, (_, at) => heroes + at)
  return { boss: heroes, enemies }
}

const PARTIES = [1, 2, 3, 4, 5, 6].flatMap((heroes) => [0, 1, 2].map((minions) => [heroes, minions] as const))

describe('arrangeSeats with a boss', () => {
  it.each(PARTIES)('%i heroes against a boss and %i minions', (heroes, minions) => {
    const seatCount = heroes + 1 + minions
    for (let viewer = 0; viewer < heroes; viewer += 1) {
      const plan = arrangeSeats({ seatCount, viewer, sides: sides(heroes, minions) })
      expect(plan.mode).toBe('boss')
      expect(plan.shape).toBe('boss')
      expect(plan.boards).toEqual(sides(heroes, minions).enemies)
      expect(plan.allies).toHaveLength(heroes - 1)
      expect(plan.allies).not.toContain(viewer)
      expect([plan.viewer, ...plan.allies, ...plan.boards].sort((a, b) => a - b)).toEqual(
        Array.from({ length: seatCount }, (_, at) => at),
      )
    }
  })

  it('a seat of the boss side, a table without a boss and a chosen mode keep the other layouts', () => {
    expect(arrangeSeats({ seatCount: 4, viewer: 3, sides: sides(3, 0) }).mode).toBe('grid')
    expect(arrangeSeats({ seatCount: 4, viewer: 0, sides: null }).mode).toBe('grid')
    expect(arrangeSeats({ seatCount: 4, viewer: 0, mode: 'boss' }).mode).toBe('grid')
    expect(arrangeSeats({ seatCount: 4, viewer: 0, sides: sides(3, 0), mode: 'focus' }).mode).toBe('focus')
  })

  it('on a phone the boss side with minions is tiles over the focused board, the boss unless a minion was picked', () => {
    const phone = { width: 390, height: 844 }
    const plan = planTable({ viewport: phone, seatCount: 6, viewer: 0, sides: sides(3, 2) })
    expect(plan.seats.tiles).toHaveLength(3)
    expect(plan.seats.boards).toEqual([3])
    expect(plan.opponentPod).toBe('none')
    const picked = planTable({ viewport: phone, seatCount: 6, viewer: 0, sides: sides(3, 2), focus: 5 })
    expect(picked.seats.boards).toEqual([5])
    const alone = planTable({ viewport: phone, seatCount: 4, viewer: 0, sides: sides(3, 0) })
    expect(alone.seats.tiles).toEqual([])
    expect(alone.opponentPod).toBe('line')
  })

  it.each([
    // [phone, opponents, own, allies, band]
    [false, 300, 300, 0, 0],
    [true, 300, 300, 4, ALLY_STRIP_PX],
    [false, 100, 100, 2, ALLY_STRIP_PX],
    [false, 400, 200, 2, ALLY_STRIP_PX],
    [false, 50, 300, 2, ALLIES_MIN_PX],
    [false, 400, 330, 2, Math.floor(330 * ALLIES_OWN_MOST)],
    [false, 2000, 2000, 2, ALLIES_MAX_PX],
  ] as const)(
    "sizes the party's band (phone %s, rows %i and %i, %i allies) as %i: a strip where boards would not fit",
    (phone, opponents, own, allies, band) => {
      expect(alliesBandPx({ phone, opponents, own }, allies)).toBe(band)
    },
  )

  it.each([200, 300, 500, 900])('keeps your board the larger part of a %i px row', (own) => {
    const band = alliesBandPx({ phone: false, opponents: own * 2, own }, 3)
    expect(band === ALLY_STRIP_PX || band <= own * ALLIES_OWN_MOST).toBe(true)
    expect(own - band).toBeGreaterThan(band)
  })
})

describe('bossSidesOf', () => {
  it("reads the boss and its team's seats from the view; none without a boss", () => {
    const view = withBoss(buildFixture({ seats: 6 }), 2).view
    expect(bossSidesOf(view)).toEqual({ boss: 3, enemies: [3, 4, 5] })
    expect(parseBossSides('3,4,5')).toEqual({ boss: 3, enemies: [3, 4, 5] })
    expect(bossSidesOf(buildFixture({ seats: 4 }).view)).toBeNull()
    expect(parseBossSides('')).toBeNull()
  })
})

describe('the table in boss mode', () => {
  it.each([
    [2, 0],
    [4, 1],
    [6, 2],
  ])('%i heroes and %i minions: the boss side on top, the party at the bottom', (heroes, minions) => {
    const fixture = withBoss(buildFixture({ seats: heroes + 1 + minions }), minions)
    const { container } = renderTable({ fixture })
    const top = container.querySelector('[data-layout="boss"]')
    expect(top).not.toBeNull()
    expect(top?.querySelector('[data-slot="boss-frame"] [data-boss]')).not.toBeNull()
    expect(top?.querySelectorAll('[data-slot="minion-frame"]')).toHaveLength(minions)
    const party = container.querySelector('[data-slot="allies"]')
    if (heroes === 1) expect(party).toBeNull()
    else expect(party?.querySelectorAll('[data-slot="ally-frame"]')).toHaveLength(heroes - 1)
    // No team chips: the sides are told by place.
    expect(container.querySelector('[data-slot="team-badge"]')).toBeNull()
  })

  it('a tile in the party row widens that hero, a second press evens the row', async () => {
    const { container, user } = renderTable({ fixture: withBoss(buildFixture({ seats: 4 }), 0) })
    const party = within(screen.getByRole('region', { name: 'Your party' }))
    const tiles = party.getAllByRole('button')
    const first = tiles[0]
    if (first === undefined) throw new Error('no tile')
    await user.click(first)
    expect(container.querySelectorAll('[data-slot="ally-frame"][data-wide]')).toHaveLength(1)
    await user.click(first)
    expect(container.querySelectorAll('[data-slot="ally-frame"][data-wide]')).toHaveLength(0)
  })
})
