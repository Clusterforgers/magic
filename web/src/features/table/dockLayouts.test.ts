// Goal: with 1 to 3 commanders waiting at the head of a hand of 0 to 12 cards, in docks from 300 to 1500 px wide and 96 to 200 px high on fine and touch pointers, the hand's line keeps at least one whole card, the commanders keep the line they need when it holds everything, and scroll in what is left (one at least in view) when it does not.
import { describe, expect, it } from 'vitest'
import { COMMAND_GAP_PX, dockLayouts, handLayout } from './handFit'

const WIDTHS = [300, 340, 362, 400, 518, 700, 998, 1500]
const HEIGHTS = [96, 122, 153, 200]
const COUNTS = Array.from({ length: 13 }, (_, index) => index)
const COMMANDERS = [1, 2, 3]

describe.each([false, true])('dockLayouts (coarse pointer %s)', (coarse) => {
  it('keeps one whole card of the hand in view beside the commanders', () => {
    for (const width of WIDTHS) {
      for (const height of HEIGHTS) {
        for (const count of COUNTS.filter((n) => n > 0)) {
          for (const commanders of COMMANDERS) {
            const where = `${count} cards and ${commanders} commanders in ${width}x${height}`
            const { hand, commandWidth = 0 } = dockLayouts({ width, height, count, commanders, coarse })
            expect(width - commandWidth, where).toBeGreaterThanOrEqual(hand.box.width)
          }
        }
      }
    }
  })

  it('gives the commanders their whole line when it fits, else lets them scroll with one in view', () => {
    for (const width of WIDTHS) {
      for (const height of HEIGHTS) {
        for (const count of COUNTS) {
          for (const commanders of COMMANDERS) {
            const where = `${count} cards and ${commanders} commanders in ${width}x${height}`
            const { command, commandWidth = 0 } = dockLayouts({ width, height, count, commanders, coarse })
            const all = handLayout({ width, height, count: count + commanders, coarse })
            const head = commanders * (all.box.width + all.gap) + COMMAND_GAP_PX
            expect(command?.perRow, where).toBe(commanders)
            expect(command?.scrolls, where).toBe(commandWidth < head)
            expect(commandWidth, where).toBeLessThanOrEqual(head)
            if (count === 0) expect(commandWidth, where).toBe(Math.min(head, width))
            if (command?.scrolls === true) {
              expect(commandWidth, where).toBeGreaterThanOrEqual(command.box.width + COMMAND_GAP_PX)
            }
          }
        }
      }
    }
  })

  it('leaves a dock without commanders to the hand alone', () => {
    const docks = dockLayouts({ width: 390, height: 122, count: 7, commanders: 0, coarse })
    expect(docks.command).toBeUndefined()
    expect(docks.hand).toEqual(handLayout({ width: 390, height: 122, count: 7, coarse }))
  })
})
