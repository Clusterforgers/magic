// Goal: the corner of a seat's pod picks the widest picture of 44 px or more that fits the pod's width and the height its column leaves, at most 0.6 of the lane card width, in as few rows as that allows, and gives no picture where the least does not fit, so the column keeps its chips instead of scrolling.
import { describe, expect, it } from 'vitest'
import {
  CORNER_GAP_PX,
  CORNER_MIN_PX,
  CORNER_ROOM_PX,
  PARTNER_HEAD_PX,
  cornerHeight,
  pictureHeight,
  pictureWidth,
  planCorner,
} from './cornerFit'
import type { CornerPlan } from './cornerFit'
import { POD_COLUMN_PX } from './layout'

describe('the picture width', () => {
  it.each<[string, number | null, number]>([
    ['not measured', null, 44],
    ['0.6 of a card of 120 px', 120, 72],
    ['0.6 of a card of 154 px', 154, 92],
    ['at least 44 px on a small card', 60, 44],
    ['at least 44 px on a tiny card', 20, 44],
    ['0.6 of the least card, rounded down', 88, 52],
  ])('is %s', (_name, card, width) => {
    expect(pictureWidth(card)).toBe(width)
  })

  it('keeps the card shape of 5 to 7', () => {
    expect([44, 48, 74, 92].map(pictureHeight)).toEqual([62, 67, 104, 129])
  })

  it('works inside the pod column: its width less the padding of both sides', () => {
    expect(CORNER_ROOM_PX).toBe(POD_COLUMN_PX - 16)
  })
})

describe('the room under the blocks of the pod', () => {
  it.each<[string, number | null, number | null, number | null]>([
    ['a column of 183 px with 74 px above', 183, 74, 85],
    ['a column not measured', null, 74, null],
    ['blocks not measured', 183, null, null],
    ['a column shorter than its blocks', 60, 74, 0],
  ])('is %s: %s', (_name, column, above, room) => {
    expect(cornerHeight(column, above)).toBe(room)
  })
})

describe('planning the corner', () => {
  const plan = (input: Parameters<typeof planCorner>[0]): CornerPlan | null => planCorner(input)

  it.each<[string, Parameters<typeof planCorner>[0], CornerPlan | null]>([
    [
      'three slots beside cards of 120 px in 85 px: one row of 48 px pictures',
      { slots: 3, width: 152, height: 85, card: 120 },
      { columns: 3, width: 48, height: 67 },
    ],
    [
      'three slots with nothing measured: one row of the least picture',
      { slots: 3, width: 152, height: null, card: null },
      { columns: 3, width: 44, height: 62 },
    ],
    [
      'three slots beside cards of 120 px with no height measured: the least picture, since the fit cannot be checked',
      { slots: 3, width: 152, height: null, card: 120 },
      { columns: 3, width: 44, height: 62 },
    ],
    [
      'two slots on a tall column beside cards of 154 px: one column of 0.6 of a card',
      { slots: 2, width: 152, height: 262, card: 154 },
      { columns: 1, width: 92, height: 129 },
    ],
    [
      'two slots in a column of 100 px: side by side, as high as the room allows',
      { slots: 2, width: 152, height: 100, card: 154 },
      { columns: 2, width: 71, height: 99 },
    ],
    [
      'three slots with a partner pair: the second head takes 16 px of the height',
      { slots: 3, width: 152, height: 85, card: 120, extra: PARTNER_HEAD_PX },
      { columns: 3, width: 48, height: 67 },
    ],
    [
      'three slots with a partner pair in 80 px: the picture gives way to the head',
      { slots: 3, width: 152, height: 80, card: 120, extra: PARTNER_HEAD_PX },
      { columns: 3, width: 45, height: 63 },
    ],
    [
      'three slots in 57 px: the least picture does not fit',
      { slots: 3, width: 152, height: 57, card: 120 },
      null,
    ],
    ['no slot', { slots: 0, width: 152, height: 200, card: 120 }, null],
    ['a pod narrower than the least picture', { slots: 3, width: 40, height: 200, card: 120 }, null],
    [
      'a small card on a tall column: the least picture, two columns of two rows are not wider',
      { slots: 3, width: 152, height: 300, card: 60 },
      { columns: 3, width: 44, height: 62 },
    ],
  ])('draws %s', (_name, input, expected) => {
    expect(plan(input)).toEqual(expected)
  })

  it('prefers fewer rows when two shapes give the same width', () => {
    // At 44 px every shape from one to three columns fits a card of 60 px; one row of three is the fewest rows.
    expect(plan({ slots: 3, width: 152, height: 400, card: 60 })?.columns).toBe(3)
  })

  it('never draws a picture outside its room, under 44 px, or over 0.6 of a card', () => {
    for (const slots of [1, 2, 3]) {
      for (const height of [null, 60, 70, 85, 100, 134, 200, 262, 400]) {
        for (const card of [null, 40, 70, 88, 120, 154, 190]) {
          for (const extra of [0, PARTNER_HEAD_PX]) {
            const found = plan({ slots, width: CORNER_ROOM_PX, height, card, extra })
            if (found === null) continue
            const rows = Math.ceil(slots / found.columns)
            expect(found.width).toBeGreaterThanOrEqual(CORNER_MIN_PX)
            expect(found.width).toBeLessThanOrEqual(pictureWidth(card))
            expect(found.columns * found.width + (found.columns - 1) * CORNER_GAP_PX).toBeLessThanOrEqual(
              CORNER_ROOM_PX,
            )
            if (height !== null) {
              expect(rows * found.height + (rows - 1) * CORNER_GAP_PX + extra).toBeLessThanOrEqual(height + 1)
            }
          }
        }
      }
    }
  })

  it('never shrinks a picture when the room grows', () => {
    for (const slots of [2, 3]) {
      let last = 0
      for (let height = 40; height <= 400; height += 4) {
        const width = plan({ slots, width: CORNER_ROOM_PX, height, card: 154 })?.width ?? 0
        expect(width).toBeGreaterThanOrEqual(last)
        last = width
      }
    }
  })
})
