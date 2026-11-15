// Goal: at a grid of opponents with 40 permanents a seat (lands folded) your row is tall enough for your creatures past 55 px at 1400x900, the opponents keep their pods and a row of cards, and `ownCrowd` never shrinks your row against the split without it.
import { describe, expect, it } from 'vitest'
import { fixtureState } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import {
  boardCounts,
  boardFrames,
  OWN_CROWD_PX,
  splitBoards,
  splitRegions,
  theirChromeOf,
} from './boardSplit'
import type { BoardCounts } from './boardSplit'
import { planTable } from './layout'
import { battlefieldStacks, foldLands, layoutSeat } from './rowStacks'

/** The counts of a grid's boards with their lands folded into the pods' chips. */
const folded = (counts: BoardCounts): BoardCounts => ({
  own: { ...counts.own, lands: 0 },
  theirs: counts.theirs.map((board) => ({ ...board, lands: 0 })),
})

function ownCardAt(width: number, height: number): { own: number; opponents: number; keep: number } {
  const state = fixtureState(buildFixture({ seats: 4, permanents: 40 }))
  const table = planTable({ viewport: { width, height }, seatCount: 4, viewer: 0, mode: 'grid' })
  const frames = boardFrames(table.regions, table.seats)
  const regions = splitRegions(table.regions, table.seats, folded(boardCounts(state, table.seats)))
  const box = { width: Math.floor(frames.own.width), height: Math.floor(regions.own - frames.own.chrome) }
  const stacks = foldLands(battlefieldStacks(state, 0))
  const fit = layoutSeat({ box, stacks, facing: 'near', opened: new Set(), marked: new Set() }).fit
  if (fit === null) throw new Error('a box was given')
  return { own: fit.cardWidth, opponents: regions.opponents, keep: theirChromeOf(frames.theirs.chrome, 0) }
}

describe('your board in a crowd', () => {
  it('draws your creatures past 55 px at 1400x900 with 40 permanents a seat', () => {
    const { own, opponents, keep } = ownCardAt(1400, 900)
    expect(own).toBeGreaterThan(55)
    expect(opponents).toBeGreaterThan(keep)
  })

  it.each([
    [600, 40],
    [450, 40],
    [800, 30],
  ])(
    'never gives your row less in %i px of room with %i creatures a seat than the split without a crowd floor',
    (room, creatures) => {
      const input = {
        room,
        ownChrome: 24,
        theirChrome: 120,
        theirScale: 0.85,
        ownWidth: 700,
        theirWidth: 300,
        own: { lands: 0, creatures, other: 10 },
        theirs: [1, 2, 3].map(() => ({ lands: 0, creatures, other: 10 })),
      }
      const plain = splitBoards(input)
      const crowded = splitBoards({ ...input, ownCrowd: OWN_CROWD_PX })
      expect(crowded.own).toBeGreaterThanOrEqual(plain.own)
      expect(crowded.opponents + crowded.own).toBeCloseTo(room, 6)
    },
  )
})
