// Goal: grouping and fitting the permanents of a whole table (40 a seat, 6 or 12 seats) takes a few milliseconds, including the second pass that merges the other cards when the first leaves them under 64 px, so the table can afford to run both on every resize and every batch.
import { describe, expect, it } from 'vitest'
import { indexById } from '@/play/view'
import type { ObjectView, PlayerId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { buildFixture } from './dev/fixture'
import { FIT_LIMITS, LAND_SCALE, fitRows } from './fit'
import type { Fit, FitBox } from './fit'
import { GROUP_OTHERS_BELOW, groupPermanents } from './group'
import { splitBattlefield } from './selectors'

/** One seat's permanents in the three rows of the battlefield. */
interface SeatRows {
  creatures: ObjectView[]
  other: ObjectView[]
  lands: ObjectView[]
}

function tableRows(seats: number): SeatRows[] {
  const { view } = buildFixture({ seats, permanents: 40 })
  const objects = indexById(view.objects)
  const bySeat = splitBattlefield(view, objects)
  const objectsOf = (ids: readonly number[]): ObjectView[] =>
    ids.map((id) => must(objects[id], `object ${id}`))
  return Array.from({ length: seats }, (_, seat: PlayerId) => {
    const rows = must(bySeat.get(seat), `seat ${seat}`)
    return {
      creatures: objectsOf(rows.creatures),
      other: objectsOf(rows.other),
      lands: objectsOf(rows.lands),
    }
  })
}

/** What the table does for a zone: fit with every card alone, and merge the other cards when that falls under 64 px. */
function arrange(rows: SeatRows, box: FitBox): { fit: Fit; merged: boolean; permanents: number } {
  let result: { fit: Fit; merged: boolean; permanents: number } | undefined
  for (const groupOthers of [false, true]) {
    const stacks = [rows.creatures, rows.other, rows.lands].map((row) =>
      groupPermanents(row, { groupOthers }),
    )
    const fit = fitRows(
      box,
      stacks.map((row, index) => ({ stacks: row.length, ...(index === 2 ? { scale: LAND_SCALE } : {}) })),
    )
    const permanents = stacks.reduce(
      (sum, row) => sum + row.reduce((count, stack) => count + stack.count, 0),
      0,
    )
    result = { fit, merged: groupOthers, permanents }
    if (fit.cardWidth >= GROUP_OTHERS_BELOW) break
  }
  return must(result)
}

const median = (samples: number[]): number =>
  must([...samples].sort((a, b) => a - b)[Math.floor(samples.length / 2)])

describe('fit and grouping over a whole table', () => {
  // The own zone of a 1280x720 table is about this big; it is small enough to need the second pass.
  const BOX: FitBox = { width: 900, height: 230 }

  it.each([
    [6, 5],
    [12, 10],
  ])('lays out %i seats of 40 permanents in under %i ms (median of 100 passes)', (seats, budget) => {
    const table = tableRows(seats)
    const pass = () => table.map((rows) => arrange(rows, BOX))

    const first = pass()
    // The work is real: every permanent is placed, every size is on the ladder, and the second pass is in the timing.
    expect(first.reduce((sum, seat) => sum + seat.permanents, 0)).toBe(seats * 40)
    for (const { fit } of first) {
      expect(fit.cardWidth).toBeGreaterThanOrEqual(FIT_LIMITS.minWidth)
      expect(fit.cardWidth).toBeLessThanOrEqual(FIT_LIMITS.maxWidth)
      expect(fit.lines).toBeGreaterThan(0)
    }
    expect(first.filter((seat) => seat.merged).length).toBeGreaterThan(0)

    for (let warm = 0; warm < 20; warm += 1) pass()
    const samples = Array.from({ length: 100 }, () => {
      const start = performance.now()
      pass()
      return performance.now() - start
    })
    expect(median(samples)).toBeLessThan(budget)
  })

  it('is the same on every pass', () => {
    const table = tableRows(6)
    const run = () => table.map((rows) => arrange(rows, BOX))
    expect(run()).toEqual(run())
  })
})
