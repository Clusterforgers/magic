//! Goal: a phone from 560 to 900 px high keeps a row of your permanents in view in every mode: a mode's own bars (the party's strip, the boss's side past a duel's head) lower the hand dock to its peek row sooner, a short phone keeps the boss's pod and both sides' tile strips to one line with the details a tap away, and a duel or team table keeps the layout it had.
import { fireEvent } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { boardCounts, boardFrames, splitRegions, theirChromeOf } from './boardSplit'
import { PHONE_FLOOR_PX } from './boardFloor'
import { fixtureState } from './dev/devGame'
import { withBoss } from './dev/devGame'
import { withTeams } from './dev/endScene'
import { buildFixture } from './dev/fixture'
import type { Fixture } from './dev/fixture'
import { laneHeight } from './laneFit'
import type { LaneCounts } from './laneFit'
import {
  ALLY_STRIP_PX,
  BOSS_ABILITIES_PX,
  bossGapPx,
  gridRows,
  HAND_PEEK_PX,
  phoneBarsPx,
  phoneHandCss,
  PHONE_HEAD_PX,
  phoneHandPx,
  planTable,
  shortPhone,
  TILE_LINE_PX,
  ZONE_GAP_PX,
} from './layout'
import { bossSidesOf } from './selectors'

const HEIGHTS = [560, 600, 640, 700, 760, 844, 900] as const

/** A table of `heroes` heroes against a boss and `minions` minions, or a duel, or two teams of two. */
type Table = 'duel' | 'teams' | { heroes: number; minions: number }

function fixtureOf(table: Table): Fixture {
  if (table === 'duel') return buildFixture({ seats: 2, board: 'duel' })
  if (table === 'teams') return withTeams(buildFixture({ seats: 4, permanents: 20 }), 'teams')
  return withBoss(buildFixture({ seats: table.heroes + 1 + table.minions }), table.minions)
}

function planOf(table: Table, height: number) {
  const fixture = fixtureOf(table)
  const sides = bossSidesOf(fixture.view)
  const viewport = { width: 390, height }
  return { fixture, table: planTable({ viewport, seatCount: fixture.view.players.length, viewer: 0, sides }) }
}

const TABLES: readonly (readonly [string, Table])[] = [
  ['a duel', 'duel'],
  ['two teams', 'teams'],
  ['a boss alone against three heroes', { heroes: 3, minions: 0 }],
  ['a boss and a minion against three heroes', { heroes: 3, minions: 1 }],
  ['a boss against one hero', { heroes: 1, minions: 0 }],
]

/** The boss's abilities over its framed pod line of a name row and a row of figures, each with its gap, on a tall phone. */
const BOSS_POD_844 = BOSS_ABILITIES_PX + ZONE_GAP_PX + 8 + 58 + ZONE_GAP_PX

describe("a phone mode's bars", () => {
  it.each([
    // [shape, allies, tiles of the boss's side, window height, px]
    ['duel', 0, 0, 600, 0],
    ['focus', 0, 0, 600, 0],
    ['columns', 0, 0, 844, 0],
    // The boss's abilities over its pod line, or over its side's one-line tiles, take no more than a duel's head.
    ['boss', 0, 0, 600, 0],
    ['boss', 0, 2, 600, 0],
    ['boss', 0, 0, 844, BOSS_POD_844 - PHONE_HEAD_PX],
    ['boss', 2, 0, 600, TILE_LINE_PX + 4],
    ['boss', 2, 0, 844, BOSS_POD_844 - PHONE_HEAD_PX + ALLY_STRIP_PX + ZONE_GAP_PX],
    // A tall phone's tiles of the boss's side keep a zones line under them, past a duel's head.
    ['boss', 0, 2, 844, BOSS_ABILITIES_PX + ZONE_GAP_PX + ALLY_STRIP_PX + ZONE_GAP_PX + 32 - PHONE_HEAD_PX],
  ] as const)(
    'of a %s table with %i allies and %i tiles at %i px take %i px',
    (shape, allies, tiles, height, px) => {
      const seats = (count: number) => Array.from({ length: count }, (_, at) => at + 1)
      expect(phoneBarsPx({ shape, allies: seats(allies), tiles: seats(tiles) }, height)).toBe(px)
    },
  )

  it.each(TABLES.flatMap(([name, table]) => HEIGHTS.map((height) => [name, table, height] as const)))(
    'lower the hand dock of %s at 390x%i by their room, the duel and the teams not at all',
    (_name, table, height) => {
      const { table: plan } = planOf(table, height)
      const bars = phoneBarsPx(plan.seats, height)
      if (typeof table === 'string') expect(bars).toBe(0)
      expect(plan.regions.hand).toBeCloseTo(phoneHandPx(height, bars), 6)
      expect(plan.regions.hand).toBeLessThanOrEqual(phoneHandPx(height))
      expect(plan.regions.hand).toBeGreaterThanOrEqual(HAND_PEEK_PX)
      // The grid's CSS row writes the same dock: a duel's is the one it always had.
      expect(gridRows(plan.regions)).toContain(phoneHandCss(bars))
      if (bars === 0) expect(gridRows(plan.regions)).toContain(phoneHandCss())
    },
  )
})

/** The lanes a phone's boards lay out: their lands fold into the pod's chip. */
const folded = (counts: LaneCounts): LaneCounts => ({ ...counts, lands: 0 })

/**
 * Where the opponents' battlefields begin, as the table measures it (`useTheirHead`): a duel's or the teams' tile and
 * lands line; the boss's abilities over its framed pod line (the name row alone on a short phone); or over the side's
 * tiles, one line on a short phone with the focused board's zones beside them, else a tile row and a zones line.
 */
function headOf(table: Table, height: number): number {
  if (typeof table === 'string') return 112
  const gap = bossGapPx(true, height)
  const short = shortPhone(height)
  const abilities = BOSS_ABILITIES_PX + gap
  if (table.minions === 0) return abilities + (short ? 4 + 32 + gap : 8 + 58 + gap)
  return abilities + (short ? TILE_LINE_PX + gap : ALLY_STRIP_PX + gap + 32)
}

describe("a phone's board rows in every mode", () => {
  /** The head of a card a press reaches: its name and cost. */
  const CARD_HEAD_PX = 44

  it.each(TABLES.flatMap(([name, table]) => HEIGHTS.map((height) => [height, name, table] as const)))(
    'at 390x%i keep your pod and a band of your cards in reach with %s',
    (height, _name, table) => {
      const { fixture, table: plan } = planOf(table, height)
      expect(plan.regions.phone).toBe(true)
      const raw = boardCounts(fixtureState(fixture), plan.seats)
      const counts = { own: folded(raw.own), theirs: raw.theirs.map(folded) }
      const head = headOf(table, height)
      const regions = splitRegions(plan.regions, plan.seats, counts, 0, head, 0)
      const frames = boardFrames(plan.regions, plan.seats)
      const band = laneHeight(frames.own.width, { lands: 0, creatures: 1, other: 0 }, PHONE_FLOOR_PX)
      // A whole band of your cards from 600 px (from 640 under boss mode's bars); below that at least their heads.
      const whole = typeof table === 'string' ? 600 : 640
      expect(regions.own - frames.own.chrome).toBeGreaterThanOrEqual(
        height >= whole ? band - 1e-6 : CARD_HEAD_PX,
      )
      expect(regions.opponents).toBeGreaterThanOrEqual(theirChromeOf(frames.theirs.chrome, head) - 1e-6)
      expect(regions.opponents + regions.own).toBeCloseTo(plan.regions.opponents + plan.regions.own, 6)
    },
  )
})

describe('boss mode on a phone', () => {
  const render = (height: number, minions = 0) =>
    renderTable({
      fixture: withBoss(buildFixture({ seats: 4 + minions }), minions),
      options: { viewport: { width: 390, height } },
    })

  it.each([
    // [window height, one-line strips and a folded boss pod]
    [600, true],
    [700, true],
    [844, false],
  ] as const)('at 390x%i draws its bars on one line each: %s', (height, short) => {
    const { container } = render(height)
    const party = container.querySelector('[aria-label="Your party"]')
    expect(party?.hasAttribute('data-line')).toBe(short)
    expect(party?.querySelectorAll('[data-line]').length ?? 0).toBe(short ? 2 : 0)
    const fold = container.querySelector('[data-slot="boss-frame"] [data-slot="pod-fold"]')
    expect(fold !== null).toBe(short)
    // Every pod line on a phone keeps its figures to one row: the boss's too.
    expect(container.querySelector('[data-slot="boss-frame"] [data-slot="pod-figures"]')).not.toBeNull()
  })

  it("opens the boss's figures with a tap on its pod's toggle, and folds them again with a second", () => {
    const { container } = render(600)
    const figures = () => container.querySelector('[data-slot="boss-frame"] [data-slot="pod-figures"]')
    const toggle = container.querySelector<HTMLButtonElement>(
      '[data-slot="boss-frame"] [data-slot="pod-fold"]',
    )
    if (toggle === null) throw new Error('no toggle')
    expect(toggle.getAttribute('aria-expanded')).toBe('false')
    expect(figures()?.classList.contains('hidden')).toBe(true)
    fireEvent.click(toggle)
    expect(toggle.getAttribute('aria-expanded')).toBe('true')
    expect(figures()?.classList.contains('hidden')).toBe(false)
    fireEvent.click(toggle)
    expect(figures()?.classList.contains('hidden')).toBe(true)
  })

  it.each([
    [600, true],
    [844, false],
  ] as const)(
    "at 390x%i with a minion draws the boss's side as tiles, one line with the focused board's zones beside them: %s",
    (height, short) => {
      const { container } = render(height, 1)
      const side = container.querySelector('[aria-label="The boss\'s side"]')
      expect(side?.hasAttribute('data-line')).toBe(short)
      expect(container.querySelector('[data-slot="strip-zones"]') !== null).toBe(short)
    },
  )
})
