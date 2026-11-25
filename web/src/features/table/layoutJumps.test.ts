//! Goal: the table holds still through a typical stretch of play: the board split and every board's card size change only when a board no longer fits or clearly has room for a larger card, never back and forth on one permanent, so the layout script stays under its budget of jumps.
import { describe, expect, it } from 'vitest'
import { boardCounts, boardFrames, holdSplit, splitBoards, splitRegions } from './boardSplit'
import type { HeldRows } from './boardSplit'
import { fixtureState } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import type { FixtureOptions } from './dev/fixture'
import { scriptStates } from './dev/layoutScript'
import { CARD_RUNGS, fitHeld, GROW_SHARE } from './holdFit'
import type { FitHold } from './holdFit'
import { fitLanes, LANE_LIMITS } from './laneFit'
import type { LaneCounts } from './laneFit'
import { planTable } from './layout'
import type { Viewport } from './layout'

/** A box moving or a card resizing by more than this is a jump, as `scripts/layout-jumps.mjs` counts them. */
const JUMP_PX = 4

interface Jumps {
  rows: number
  cards: number
}

/**
 * The jumps of the layout script over `options` in `viewport`: the opponents' row and your row as the split gives
 * them, and the card width of your board and of the first opponent's in the boxes the split leaves. `held` keeps
 * the split's rows and the boards' cards (`HeldRows`, `fitHeld`); without it every state is fitted afresh.
 */
function jumpsOf(options: FixtureOptions, viewport: Viewport, held: boolean): Jumps {
  const plan = planTable({ viewport, seatCount: options.seats, viewer: 0 })
  const states = scriptStates(buildFixture(options))
  let rows: HeldRows | null = null
  const holds: (FitHold | null)[] = [null, null]
  let last: number[] | null = null
  const jumps = { rows: 0, cards: 0 }
  for (const step of states) {
    const counts = boardCounts(fixtureState(step.fixture, step.pending), plan.seats)
    const regions = splitRegions(plan.regions, plan.seats, counts, 0, 0, 0, held ? rows : null)
    rows = { opponents: regions.opponents, own: regions.own }
    const frames = boardFrames(regions, plan.seats)
    const boards: [LaneCounts, number, number, 'near' | 'far'][] = [
      [counts.own, frames.own.width, regions.own - frames.own.chrome, 'near'],
      [counts.theirs[0] ?? counts.own, frames.theirs.width, regions.opponents - frames.theirs.chrome, 'far'],
    ]
    const cards = boards.map(([lanes, width, height, facing], at) => {
      const box = { width, height: Math.max(0, height) }
      const fit = held
        ? fitHeld(box, lanes, facing, LANE_LIMITS, holds[at] ?? null)
        : fitLanes(box, lanes, facing)
      holds[at] = { width: fit.cardWidth, shape: fit.shape }
      return fit.cardWidth
    })
    const now = [regions.opponents, ...cards]
    if (last !== null) {
      if (Math.abs((now[0] ?? 0) - (last[0] ?? 0)) > JUMP_PX) jumps.rows += 1
      for (let at = 1; at < now.length; at += 1) {
        if (Math.abs((now[at] ?? 0) - (last[at] ?? 0)) > JUMP_PX) jumps.cards += 1
      }
    }
    last = now
  }
  return jumps
}

const TABLES: [string, FixtureOptions][] = [
  ['a duel', { seats: 2, board: 'duel' }],
  ['four seats', { seats: 4, permanents: 20 }],
]
const SIZES: Viewport[] = [
  { width: 1873, height: 1047 },
  { width: 1400, height: 900 },
  { width: 390, height: 844 },
  { width: 390, height: 700 },
  { width: 390, height: 600 },
]
/** The most jumps of rows and cards together the thirty states may make at one table and size. */
const BUDGET = 6

describe('the layout script', () => {
  it.each(TABLES.flatMap(([name, options]) => SIZES.map((size) => [name, options, size] as const)))(
    'moves %s at %o within the budget, fewer times than fitting every state afresh',
    (_, options, size) => {
      const held = jumpsOf(options, size, true)
      const fresh = jumpsOf(options, size, false)
      expect(held.rows + held.cards).toBeLessThanOrEqual(BUDGET)
      expect(held.rows + held.cards).toBeLessThanOrEqual(fresh.rows + fresh.cards)
    },
  )
})

describe('fitHeld', () => {
  const box = { width: 900, height: 300 }
  const lanes = (creatures: number): LaneCounts => ({ lands: 6, creatures, other: 2 })

  it('steps between rungs of the card ladder only', () => {
    for (let creatures = 1; creatures < 30; creatures += 1) {
      expect(CARD_RUNGS).toContain(fitHeld(box, lanes(creatures), 'near', LANE_LIMITS, null).cardWidth)
    }
  })

  it.each([4, 8, 12, 16, 20])('keeps its card when %i creatures gain one and lose it again', (creatures) => {
    const start = fitHeld(box, lanes(creatures), 'near', LANE_LIMITS, null)
    const hold = { width: start.cardWidth, shape: start.shape }
    const more = fitHeld(box, lanes(creatures + 1), 'near', LANE_LIMITS, hold)
    const back = fitHeld(box, lanes(creatures), 'near', LANE_LIMITS, {
      width: more.cardWidth,
      shape: more.shape,
    })
    // At most one step down on the way there, none on the way back: no flip-flop.
    expect(back.cardWidth).toBe(more.cardWidth)
    expect(back.shape).toBe(more.shape)
  })

  it('steps down as soon as the held card no longer fits', () => {
    const big = fitHeld(box, lanes(2), 'near', LANE_LIMITS, null)
    const crowded = fitHeld(box, lanes(24), 'near', LANE_LIMITS, { width: big.cardWidth, shape: big.shape })
    expect(crowded.cardWidth).toBeLessThan(big.cardWidth)
    expect(crowded.height).toBeLessThanOrEqual(box.height)
  })

  it('grows only into clearly more room', () => {
    const crowded = fitHeld(box, lanes(24), 'near', LANE_LIMITS, null)
    const hold = { width: crowded.cardWidth, shape: crowded.shape }
    const emptier = fitHeld(box, lanes(4), 'near', LANE_LIMITS, hold)
    expect(emptier.cardWidth).toBeGreaterThanOrEqual(crowded.cardWidth * (1 + GROW_SHARE))
    const fresh = fitLanes(box, lanes(23), 'near')
    const nearly = fitHeld(box, lanes(23), 'near', LANE_LIMITS, hold)
    if (fresh.cardWidth < crowded.cardWidth * (1 + GROW_SHARE))
      expect(nearly.cardWidth).toBe(crowded.cardWidth)
  })
})

describe('holdSplit', () => {
  const base = { room: 700, ownChrome: 24, theirChrome: 24, ownWidth: 1100, theirWidth: 1100 }
  const counts = (lands: number, creatures: number, other: number): LaneCounts => ({
    lands,
    creatures,
    other,
  })

  it.each([
    [counts(6, 4, 2), counts(6, 4, 2)],
    [counts(8, 9, 3), counts(7, 7, 4)],
    [counts(10, 14, 5), counts(9, 12, 6)],
  ])('keeps its rows when %o gains a creature and loses it again', (own, theirs) => {
    const start = splitBoards({ ...base, own, theirs: [theirs] })
    const more = holdSplit(
      { ...base, own: { ...own, creatures: own.creatures + 1 }, theirs: [theirs] },
      start,
    )
    const back = holdSplit({ ...base, own, theirs: [theirs] }, more)
    expect(back.opponents).toBe(more.opponents)
    expect(more.opponents).toBe(start.opponents)
  })

  it('gives way to the fresh split when the held rows no longer fit', () => {
    const start = splitBoards({ ...base, own: counts(2, 1, 0), theirs: [counts(20, 20, 10)] })
    const crowded = { ...base, own: counts(20, 30, 10), theirs: [counts(20, 20, 10)] }
    expect(holdSplit(crowded, start)).toEqual(splitBoards(crowded))
  })

  it('gives way to a clearly larger card that one permanent more would keep', () => {
    const start = splitBoards({ ...base, own: counts(20, 30, 10), theirs: [counts(2, 2, 1)] })
    const emptied = { ...base, own: counts(2, 2, 1), theirs: [counts(2, 2, 1)] }
    expect(holdSplit(emptied, start)).toEqual(splitBoards(emptied))
  })
})
