//! Goal: a phone from 560 to 900 px high keeps a row of your permanents in view: the hand dock gives way to a peek row first, the opponents' board next, and your row keeps your pod line and a band of floor cards.
import { describe, expect, it } from 'vitest'
import { boardCounts, boardFrames, splitRegions, theirChromeOf } from './boardSplit'
import { PHONE_FLOOR_PX } from './boardFloor'
import { fixtureState } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { laneHeight } from './laneFit'
import type { LaneCounts } from './laneFit'
import {
  clampedHeight,
  HAND_PEEK_PX,
  HAND_PHONE_CLAMP,
  handSink,
  PEEK_ROOM_PX,
  phoneHandPx,
  planTable,
} from './layout'

const HEIGHTS = [560, 600, 640, 700, 760, 844, 900] as const

describe("a phone's hand dock", () => {
  it.each([
    [560, HAND_PEEK_PX],
    [600, HAND_PEEK_PX],
    [652, HAND_PEEK_PX],
    [680, 84],
    [700, 104],
    [844, clampedHeight(HAND_PHONE_CLAMP, 844)],
    [900, 140],
  ])('is %i px high in a window %i px high', (height, hand) => {
    expect(phoneHandPx(height)).toBeCloseTo(hand, 6)
  })

  it('never grows as the window shrinks, and stays between the peek row and its clamp', () => {
    let last = Infinity
    for (let height = 900; height >= 400; height -= 4) {
      const hand = phoneHandPx(height)
      expect(hand).toBeLessThanOrEqual(last)
      expect(hand).toBeGreaterThanOrEqual(HAND_PEEK_PX)
      expect(hand).toBeLessThanOrEqual(HAND_PHONE_CLAMP.max)
      last = hand
    }
  })

  it.each([
    // [the line's room, a phone, how far the cards sink under the dock's edge]
    [PEEK_ROOM_PX, true, 0],
    [PEEK_ROOM_PX + 40, true, 0],
    [HAND_PEEK_PX - 9, true, PEEK_ROOM_PX - HAND_PEEK_PX + 9],
    [60, true, PEEK_ROOM_PX - 60],
    [60, false, 0],
  ])('sinks its cards in a line of %i px (phone %s) by %i px', (room, peek, sink) => {
    expect(handSink(room, peek)).toBe(sink)
  })
})

/** The lanes a phone's boards lay out: their lands fold into the pod's chip. */
const folded = (counts: LaneCounts): LaneCounts => ({ ...counts, lands: 0 })

describe("a short phone's board rows", () => {
  const tables = [
    ['a duel', 2, 'duel'],
    ['four seats', 4, 'heavy'],
  ] as const
  const cases = tables.flatMap(([name, seats, board]) =>
    HEIGHTS.map((height) => [height, name, seats, board] as const),
  )
  /** The head of a card a press reaches: its name and cost. */
  const CARD_HEAD_PX = 44
  /** Where the opponents' battlefields begin under their tile and lands line, as the table measures (`useTheirHead`). */
  const HEAD_PX = 112

  it.each(cases)(
    'at 390x%i keeps your pod and your cards in reach, and the pods of %s whole',
    (height, _name, seats, board) => {
      const fixture = buildFixture({ seats, board, permanents: board === 'heavy' ? 20 : undefined })
      const state = fixtureState(fixture)
      const table = planTable({ viewport: { width: 390, height }, seatCount: seats, viewer: 0 })
      expect(table.regions.phone).toBe(true)
      const raw = boardCounts(state, table.seats)
      const counts = { own: folded(raw.own), theirs: raw.theirs.map(folded) }
      const regions = splitRegions(table.regions, table.seats, counts, 0, HEAD_PX, 0)
      const frames = boardFrames(table.regions, table.seats)
      const band = laneHeight(frames.own.width, { lands: 0, creatures: 1, other: 0 }, PHONE_FLOOR_PX)
      // From 600 px a whole band of your cards; below it at least their heads.
      expect(regions.own - frames.own.chrome).toBeGreaterThanOrEqual(
        height >= 600 ? band - 1e-6 : CARD_HEAD_PX,
      )
      expect(regions.opponents).toBeGreaterThanOrEqual(theirChromeOf(frames.theirs.chrome, HEAD_PX) - 1e-6)
      expect(regions.opponents + regions.own).toBeCloseTo(table.regions.opponents + table.regions.own, 6)
    },
  )
})
