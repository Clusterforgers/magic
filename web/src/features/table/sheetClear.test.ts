//! Goal: the prompt band never covers an opponent's or the boss's cards and the boss's board stays readable: in every layout (a duel, four seats, a grid, focus, the boss with one to six heroes, the phone) and window size the band is the strip itself (on a phone with a second line, whose room the split keeps free below the opponents' boards whatever the prompt), the boards keep a row of cards above a sheet that grows, and the boss's cards are at least 60 % of the heroes' where its board is like theirs.
import { describe, expect, it } from 'vitest'
import { BOARD_KEEP_PX, boardCounts, boardFrames, splitRegions } from './boardSplit'
import { fixtureState, withBoss } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { DEV_PROMPTS } from './dev/prompts'
import { BAND_PHONE_PX, planTable } from './layout'
import type { TableMode, Viewport } from './layout'
import { bossSidesOf } from './selectors'

const SIZES: Viewport[] = [
  { width: 1881, height: 1047 },
  { width: 1400, height: 900 },
  { width: 1024, height: 768 },
  { width: 390, height: 844 },
]

/** The tables: seats, a mode the viewer picked, the heroes of a boss fight (0 for none), permanents a seat. */
const TABLES: [string, number, TableMode | undefined, boolean, number][] = [
  ['a duel', 2, undefined, false, 16],
  ['four seats', 4, undefined, false, 20],
  ['a grid of four', 4, 'grid', false, 16],
  ['focus at six', 6, 'focus', false, 16],
  ...[1, 2, 3, 4, 5, 6].map((heroes): [string, number, TableMode | undefined, boolean, number] => [
    `the boss and ${heroes} heroes`,
    heroes + 1,
    undefined,
    true,
    16,
  ]),
]

/** The table's regions split for a fixture during a payment, as `useBoardSplit` draws them. */
function tableOf(
  seats: number,
  mode: TableMode | undefined,
  boss: boolean,
  permanents: number,
  viewport: Viewport,
) {
  const base = buildFixture({ seats, permanents })
  const fixture = boss ? withBoss(base, 0) : base
  const state = fixtureState(fixture, DEV_PROMPTS['pay']?.(fixture) ?? null)
  const plan = planTable({
    viewport,
    seatCount: seats,
    viewer: 0,
    mode,
    sides: boss ? bossSidesOf(state.view) : null,
  })
  const regions = splitRegions(plan.regions, plan.seats, boardCounts(state, plan.seats))
  return { plan, regions }
}

describe('the prompt sheet over the opponents', () => {
  it.each(TABLES.flatMap((table) => SIZES.map((size) => [...table, size] as const)))(
    'covers no card of %s at %#: it stays in the room the boards keep free',
    (_name, seats, mode, boss, permanents, viewport) => {
      const { plan, regions } = tableOf(seats, mode, boss, permanents, viewport)
      // One line is the strip; a phone's second line is kept above it.
      expect(regions.sheetRoom).toBe(regions.phone ? BAND_PHONE_PX - regions.shared : 0)
      // The sheet's height over the strip is never more than the room kept below the opponents' boards.
      expect(regions.sheet - regions.shared).toBeLessThanOrEqual(regions.sheetRoom + 1e-9)
      // Above that room the opponents' row keeps its chrome and a row of cards, which a sheet of cards leaves too.
      expect(regions.sheetWide).toBeGreaterThanOrEqual(regions.sheet)
      if (!regions.phone && plan.seats.shape !== 'boss') {
        const chrome = boardFrames(regions, plan.seats).theirs.chrome
        expect(regions.opponents - regions.sheetRoom).toBeGreaterThanOrEqual(chrome + BOARD_KEEP_PX - 1e-9)
        expect(regions.opponents - (regions.sheetWide - regions.shared)).toBeGreaterThanOrEqual(
          Math.min(regions.opponents, chrome + BOARD_KEEP_PX) - 1e-9,
        )
      }
    },
  )

  it.each(SIZES)('keeps one band at %o for every prompt kind, so no prompt moves a board', (viewport) => {
    const fixture = buildFixture({ seats: 4, permanents: 20 })
    const plan = planTable({ viewport, seatCount: 4, viewer: 0 })
    const rows = new Set<string>()
    for (const name of ['priority', 'pay', 'scry', 'targets', 'mode-two', 'x', 'cards']) {
      const state = fixtureState(fixture, DEV_PROMPTS[name]?.(fixture) ?? null)
      const split = splitRegions(plan.regions, plan.seats, boardCounts(state, plan.seats))
      rows.add(`${split.opponents}|${split.own}|${split.sheet}`)
    }
    expect(rows.size).toBe(1)
  })
})

describe("the boss's share of the table", () => {
  it.each([1, 2, 3, 4, 5, 6].flatMap((heroes) => SIZES.slice(0, 3).map((size) => [heroes, size] as const)))(
    'draws the boss at least 60 percent of the heroes beside %i heroes at %o',
    (heroes, viewport) => {
      const { regions } = tableOf(heroes + 1, undefined, true, 16, viewport)
      const party = regions.party
      if (party === undefined || party.heroCard === 0) return
      expect(party.bossCard).toBeGreaterThanOrEqual(party.heroCard * 0.6)
    },
  )
})
