// Goal: while a decision picks among the hand's cards, the hand dock gives the hand its room first in docks from 300 to 1500 px wide and 96 to 240 px high, with 1 to 12 cards beside 1 to 12 commanders or Satchel cards on fine and touch pointers: the hand scrolls only where it would beside one card of the head, the head keeps one card in view and scrolls in what is left, the line never runs past the dock, a line that holds everything stays as it is, and the dock grows for the rows of the hand alone.
import { describe, expect, it } from 'vitest'
import { COMMAND_GAP_PX, dockLayouts, HAND_FLOOR_PX, HAND_GAP_PX, handDockNeed, handLayout } from './handFit'
import type { HandLayout } from './handFit'

const WIDTHS = [300, 400, 518, 657, 700, 998, 1500]
const HEIGHTS = [96, 122, 153, 200, 240]
const COUNTS = [1, 2, 5, 8, 12]
const HEADS = [1, 2, 3, 8, 12]

/** Every dock of the table, each with a name for a failing case. */
const cases = WIDTHS.flatMap((width) =>
  HEIGHTS.flatMap((height) =>
    COUNTS.flatMap((count) =>
      HEADS.map((commanders) => ({
        width,
        height,
        count,
        commanders,
        where: `${count} cards and ${commanders} at the head in ${width}x${height}`,
      })),
    ),
  ),
)

/** The width a hand layout's cards take in px. */
const rowPx = (layout: HandLayout): number =>
  layout.perRow * layout.box.width + (layout.perRow - 1) * layout.gap

describe.each([false, true])('dockLayouts while picking among the hand (coarse pointer %s)', (coarse) => {
  it('lays the hand out first and scrolls it only where one card of the head leaves it too little', () => {
    for (const { width, height, count, commanders, where } of cases) {
      const picked = dockLayouts({ width, height, count, commanders, coarse, pick: true })
      const shared = dockLayouts({ width, height, count, commanders, coarse })
      const beside = handLayout({
        width: width - (HAND_FLOOR_PX + HAND_GAP_PX + COMMAND_GAP_PX),
        height,
        count,
        coarse,
      })
      if (shared.hand.scrolls || shared.command?.scrolls === true) {
        expect(picked.hand.scrolls, where).toBe(beside.scrolls)
      } else {
        expect(picked, where).toEqual(shared)
      }
    }
  })

  it('keeps one card of the head in view, scrolls the rest and never runs past the dock', () => {
    for (const { width, height, count, commanders, where } of cases) {
      const shared = dockLayouts({ width, height, count, commanders, coarse })
      // A line that holds everything is the shared one (the test above), sized by the shared rule.
      if (!shared.hand.scrolls && shared.command?.scrolls !== true) continue
      const {
        hand,
        command,
        commandWidth = 0,
      } = dockLayouts({
        width,
        height,
        count,
        commanders,
        coarse,
        pick: true,
      })
      const card = hand.box.width + hand.gap
      const head = commanders * card + COMMAND_GAP_PX
      expect(command?.perRow, where).toBe(commanders)
      expect(command?.rows, where).toBe(1)
      expect(commandWidth, where).toBeGreaterThanOrEqual(Math.min(head, card + COMMAND_GAP_PX))
      expect(commandWidth, where).toBeLessThanOrEqual(head)
      expect(command?.scrolls, where).toBe(commandWidth < head)
      if (!hand.scrolls) expect(commandWidth + rowPx(hand), where).toBeLessThanOrEqual(width)
    }
  })

  it('is the shared line for an empty hand', () => {
    expect(dockLayouts({ width: 500, height: 150, count: 0, commanders: 4, coarse, pick: true })).toEqual(
      dockLayouts({ width: 500, height: 150, count: 0, commanders: 4, coarse }),
    )
  })
})

describe('handDockNeed while picking among the hand', () => {
  it.each([
    { width: 657, count: 12, commanders: 12 },
    { width: 657, count: 8, commanders: 10 },
    { width: 520, count: 9, commanders: 3 },
    { width: 1000, count: 20, commanders: 6 },
  ])(
    'counts one card of the head: $count cards beside $commanders in $width px',
    ({ width, count, commanders }) => {
      const most = 400
      expect(handDockNeed({ width, count, commanders, most, pick: true })).toBe(
        handDockNeed({ width, count, commanders: 1, most }),
      )
    },
  )

  it('counts the whole head for an empty hand', () => {
    expect(handDockNeed({ width: 657, count: 0, commanders: 12, most: 400, pick: true })).toBe(
      handDockNeed({ width: 657, count: 0, commanders: 12, most: 400 }),
    )
  })
})
