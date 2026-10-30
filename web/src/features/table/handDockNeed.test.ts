// Goal: the hand dock grows only for a hand that one row of the narrowest card does not hold, to the height of the two or three rows that do (never past its most), and at that height the hand's layout holds every card without scrolling, commanders at its head included.
import { describe, expect, it } from 'vitest'
import {
  COMMAND_GAP_PX,
  HAND_DOCK_CHROME_PX,
  HAND_FLOOR_PX,
  HAND_GAP_PX,
  handDockNeed,
  handLayout,
} from './handFit'

const TWO_ROWS = 95 + 54 + HAND_DOCK_CHROME_PX
const THREE_ROWS = 95 + 2 * 54 + HAND_DOCK_CHROME_PX

describe('handDockNeed', () => {
  it.each([
    // width, cards, commanders, most, need
    [921, 7, 2, 248, 0],
    [921, 14, 2, 248, TWO_ROWS],
    [921, 16, 2, 248, TWO_ROWS],
    [601, 7, 2, 230, TWO_ROWS],
    [601, 9, 2, 230, TWO_ROWS],
    [601, 14, 2, 230, THREE_ROWS],
    [601, 16, 2, 230, THREE_ROWS],
    [601, 24, 2, 230, 0],
    [626, 7, 2, 207, TWO_ROWS],
    [626, 8, 2, 207, TWO_ROWS],
    [626, 14, 2, 207, 0],
    [514, 7, 2, 176, TWO_ROWS],
    [514, 12, 2, 176, 0],
    [800, 9, 0, 230, 0],
    [400, 9, 0, 0, 0],
  ])(
    'a %i px line with %i cards and %i commanders, at most %i px, needs %i px',
    (width, count, commanders, most, need) => {
      expect(handDockNeed({ width, count, commanders, most })).toBe(need)
    },
  )

  it.each([
    [601, 14, 2],
    [601, 7, 2],
    [921, 16, 2],
    [500, 8, 0],
    [514, 7, 2],
  ])(
    'a %i px line with %i cards and %i commanders holds them at the height it needs',
    (width, count, commanders) => {
      const need = handDockNeed({ width, count, commanders, most: 1000 })
      expect(need).toBeGreaterThan(0)
      const head = commanders > 0 ? commanders * (HAND_FLOOR_PX + HAND_GAP_PX) + COMMAND_GAP_PX : 0
      const layout = handLayout({ width: width - head, height: need - HAND_DOCK_CHROME_PX, count })
      expect(layout.scrolls).toBe(false)
      expect(layout.box.width).toBeGreaterThanOrEqual(HAND_FLOOR_PX)
    },
  )
})
