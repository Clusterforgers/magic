// Goal: the card preview always keeps the size `previewFit` gives it and only moves: to a place clear of every kept box and board card, else clear of the answers (over the sentence, the strip or cards), else over the fewest answers with the primary one uncovered, over a pod only after its own card, on every window from a phone to a wide desktop.
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import type { Anchor, Viewport } from './preview'
import { fitClear, panelBox } from './previewClear'
import type { Box, ClearBox } from './previewClear'
import { previewFit } from './previewFit'

const rect = (left: number, top: number, right: number, bottom: number): Anchor => ({
  left,
  top,
  width: right - left,
  height: bottom - top,
})

const meets = (a: Box, b: Anchor): boolean =>
  a.left < b.left + b.width && a.right > b.left && a.top < b.top + b.height && a.bottom > b.top

const touches = (fit: ReturnType<typeof fitClear>, view: Viewport, clear: readonly Anchor[]): boolean =>
  clear.some((box) => meets(panelBox(fit, view), box))

/** The fit keeps the unconstrained size: the same picture and panel width as `previewFit`. */
const fullSize = (fit: ReturnType<typeof fitClear>, card: Anchor, view: Viewport): void => {
  const base = previewFit(card, view)
  expect(fit.picture).toBe(base.picture)
  expect(fit.width).toBe(base.width)
  expect(fit.beside).toBe(base.beside)
}

/**
 * The viewer's own turn as the table measures it: the strip (soft) across the table above the own board, its sentence
 * at the left, the hand dock, and the controls at the bottom right holding End turn, the menu and the primary answer.
 */
function ownTurn(view: Viewport): { clear: ClearBox[]; primary: Anchor; answers: Anchor[] } {
  const { width, height } = view
  const side = Math.round(width * 0.17)
  const right = width - side - 8
  const stripTop = Math.round(height * 0.54)
  const strip = rect(184, stripTop, right, stripTop + 64)
  const sentence = rect(222, stripTop + 10, 540, stripTop + 30)
  const dockTop = height - Math.round(Math.min(200, Math.max(104, height * 0.16)))
  const hand = rect(0, dockTop, right - 300, height)
  const primary = rect(right - 120, height - 52, right - 4, height - 8)
  const menu = rect(right - 170, height - 52, right - 128, height - 8)
  const end = rect(right - 260, height - 52, right - 178, height - 8)
  return {
    clear: [
      { ...strip, soft: true },
      { ...sentence, kind: 'text' },
      hand,
      { ...end, kind: 'answer' },
      { ...menu, kind: 'answer' },
      { ...primary, kind: 'primary' },
    ],
    primary,
    answers: [end, menu, primary],
  }
}

/** A target prompt on the viewer's turn: the strip holds the choices with Cancel, Undo and Confirm at its top right. */
function targets(view: Viewport): { clear: ClearBox[]; primary: Anchor; answers: Anchor[] } {
  const { width, height } = view
  const right = width - Math.round(width * 0.17) - 8
  const top = Math.round(height * 0.37)
  const strip = rect(184, top, right, top + 216)
  const primary = rect(right - 150, top + 10, right - 14, top + 50)
  const undo = rect(right - 230, top + 10, right - 158, top + 50)
  const cancel = rect(right - 320, top + 10, right - 238, top + 50)
  const dockTop = height - Math.round(Math.min(200, Math.max(104, height * 0.16)))
  return {
    clear: [
      { ...strip, soft: true },
      { ...rect(560, top + 14, right - 330, top + 40), kind: 'text' },
      { ...cancel, kind: 'answer' },
      { ...undo, kind: 'answer' },
      { ...primary, kind: 'primary' },
      rect(0, dockTop, right - 300, height),
    ],
    primary,
    answers: [cancel, undo, primary],
  }
}

const WINDOWS: [number, number][] = [
  [1400, 900],
  [1530, 898],
  [1873, 1047],
]

describe('fitClear on the viewer own turn', () => {
  // The cards hovered: one at either end of the own board and one in an opponent's board.
  const cards = (view: Viewport): Anchor[] => {
    const right = view.width - Math.round(view.width * 0.17) - 8
    const row = Math.round(view.height * 0.62)
    return [
      rect(200, row, 260, row + 70),
      rect(right - 70, row, right - 10, row + 70),
      rect(600, 220, 660, 290),
    ]
  }

  it.each(WINDOWS)('at %i x %i keeps the full size and the primary answer uncovered', (width, height) => {
    const view = { width, height }
    const turn = ownTurn(view)
    for (const card of cards(view)) {
      const fit = fitClear(card, view, turn.clear)
      fullSize(fit, card, view)
      expect(touches(fit, view, [turn.primary])).toBe(false)
      expect(touches(fit, view, turn.answers)).toBe(false)
      // The sentence stays readable too: the panel covers the card it shows before it.
      expect(
        touches(
          fit,
          view,
          turn.clear.filter((box) => box.kind === 'text'),
        ),
      ).toBe(false)
    }
  })

  it.each(WINDOWS)(
    'at %i x %i keeps the full size and the answers of a target prompt uncovered',
    (width, height) => {
      const view = { width, height }
      const prompt = targets(view)
      for (const card of cards(view)) {
        const fit = fitClear(card, view, prompt.clear)
        fullSize(fit, card, view)
        expect(touches(fit, view, prompt.answers)).toBe(false)
      }
    },
  )

  it.each(WINDOWS)(
    'at %i x %i keeps the full size beside the board cards, pinned or hovered',
    (width, height) => {
      const view = { width, height }
      const turn = ownTurn(view)
      const board = Array.from({ length: 10 }, (_, at) => rect(200 + at * 90, 120, 280 + at * 90, 220))
      for (const card of cards(view)) {
        const fit = fitClear(card, view, turn.clear, board)
        fullSize(fit, card, view)
        expect(touches(fit, view, turn.answers)).toBe(false)
      }
    },
  )
})

describe('fitClear picks the place in order', () => {
  const view = { width: 1400, height: 900 }
  const card = rect(200, 300, 260, 370)

  it('is previewFit when nothing is in the way, and ignores a box without size', () => {
    expect(fitClear(card, view, [])).toEqual(previewFit(card, view))
    expect(fitClear(card, view, [rect(1100, 300, 1100, 700), rect(0, 0, 0, 0)])).toEqual(
      previewFit(card, view),
    )
  })

  it('moves above a strip that would lie under the natural panel when the room above holds the picture', () => {
    const clear: ClearBox[] = [{ ...rect(184, 700, 1200, 760), soft: true }]
    const fit = fitClear(card, view, clear)
    fullSize(fit, card, view)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit).toMatchObject({ dock: 'right', top: 64, maxHeight: 700 - 8 - 64 })
  })

  it('takes a place clear of every box and card before one over cards', () => {
    const shown = rect(600, 300, 660, 370)
    const clear: ClearBox[] = [{ ...rect(1100, 64, 1300, 200), kind: 'primary' }]
    const board = [rect(1000, 300, 1100, 400)]
    expect(previewFit(shown, view).dock).toBe('right')
    const fit = fitClear(shown, view, clear, board)
    fullSize(fit, shown, view)
    expect(fit.dock).toBe('left')
    expect(touches(fit, view, [...clear, ...board, shown])).toBe(false)
  })

  describe('at a rail window near 1024 px, with a pod column on the left and cards on both sides', () => {
    const rail = { width: 1024, height: 768 }
    const pods: ClearBox[] = [
      { ...rect(8, 52, 176, 216), kind: 'pod' },
      { ...rect(8, 378, 176, 612), kind: 'pod' },
    ]
    const board = [rect(190, 64, 970, 220), rect(190, 380, 970, 630)]
    it.each([
      // [card's left edge, dock, covers a pod]
      [400, 'right', false],
      [620, 'right', false],
      [900, 'left', true],
    ] as const)('a card at %i docks the panel %s, over a pod: %s', (left, dock, overPod) => {
      const shown = rect(left, 400, left + 60, 484)
      const fit = fitClear(shown, rail, pods, board)
      fullSize(fit, shown, rail)
      expect(fit.dock).toBe(dock)
      // The card shown outranks a pod: the panel covers a pod before it covers its own card.
      expect(touches(fit, rail, [shown])).toBe(false)
      expect(touches(fit, rail, pods)).toBe(overPod)
    })

    it('draws a panel of at most 30 % of the window', () => {
      expect(must(previewFit(rect(400, 400, 460, 484), rail).width)).toBeLessThanOrEqual(1024 * 0.3)
    })
  })

  it('covers the sentence and the strip rather than an answer', () => {
    // Answers fill the right column, the sentence and the strip the left one.
    const clear: ClearBox[] = [
      { ...rect(1000, 64, 1390, 880), kind: 'answer' },
      { ...rect(0, 300, 500, 360), kind: 'text' },
      { ...rect(0, 360, 500, 600), soft: true },
    ]
    const fit = fitClear(rect(700, 300, 760, 370), view, clear)
    fullSize(fit, rect(700, 300, 760, 370), view)
    expect(fit.dock).toBe('left')
  })

  it('covers the fewest answers and keeps the primary one uncovered where no place clears them all', () => {
    // A column of answers on each side: three on the right with the primary, two on the left.
    const clear: ClearBox[] = [
      { ...rect(1100, 100, 1300, 140), kind: 'primary' },
      { ...rect(1100, 400, 1300, 440), kind: 'answer' },
      { ...rect(1100, 700, 1300, 740), kind: 'answer' },
      { ...rect(100, 300, 300, 340), kind: 'answer' },
      { ...rect(100, 600, 300, 640), kind: 'answer' },
    ]
    const middle = rect(700, 300, 760, 370)
    const fit = fitClear(middle, view, clear)
    fullSize(fit, middle, view)
    expect(touches(fit, view, [must(clear[0])])).toBe(false)
    expect(clear.filter((box) => touches(fit, view, [box]))).toHaveLength(1)
  })

  it('covers another answer rather than the primary one', () => {
    const clear: ClearBox[] = [
      { ...rect(1100, 64, 1390, 880), kind: 'primary' },
      { ...rect(12, 64, 400, 880), kind: 'answer' },
    ]
    const middle = rect(700, 300, 760, 370)
    const fit = fitClear(middle, view, clear)
    fullSize(fit, middle, view)
    expect(fit.dock).toBe('left')
  })

  it('never covers the card it shows while another place exists', () => {
    const clear: ClearBox[] = [{ ...rect(0, 64, 500, 880), kind: 'answer' }]
    const right = rect(1200, 700, 1260, 770)
    const fit = fitClear(right, view, clear)
    fullSize(fit, right, view)
    expect(touches(fit, view, [right])).toBe(false)
  })

  it('keeps the full size and the primary answer over a sweep of windows, cards and strips', () => {
    for (let width = 1024; width <= 2300; width += 128) {
      for (let height = 640; height <= 1200; height += 80) {
        const view = { width, height }
        for (const prompt of [ownTurn(view), targets(view)]) {
          for (const x of [200, width / 2, width - 400]) {
            const shown = rect(x, height * 0.6, x + 60, height * 0.6 + 70)
            const fit = fitClear(shown, view, prompt.clear)
            fullSize(fit, shown, view)
            expect(touches(fit, view, [prompt.primary])).toBe(false)
          }
        }
      }
    }
  })
})

describe('fitClear on a Delves boss table with a strip edge to edge', () => {
  const CASES: [Viewport, Anchor, Anchor][] = [
    [{ width: 1830, height: 855 }, rect(8, 380, 1822, 600), rect(1500, 400, 1700, 580)],
    [{ width: 2300, height: 840 }, rect(8, 330, 2292, 560), rect(1900, 350, 2100, 540)],
    [{ width: 1270, height: 1380 }, rect(8, 245, 1262, 645), rect(560, 300, 710, 520)],
  ]

  it.each(CASES)(
    'at %o keeps the full size, the primary answer clear and covers one answer at most',
    (view, strip, card) => {
      const primary = rect(
        strip.left + strip.width - 140,
        strip.top + 12,
        strip.left + strip.width - 12,
        strip.top + 52,
      )
      const undo = rect(strip.left + 12, strip.top + 12, strip.left + 100, strip.top + 52)
      const clear: ClearBox[] = [
        { ...strip, soft: true },
        { ...primary, kind: 'primary' },
        { ...undo, kind: 'answer' },
        rect(0, view.height - 135, view.width * 0.6, view.height),
      ]
      const fit = fitClear(card, view, clear)
      fullSize(fit, card, view)
      expect(touches(fit, view, [primary])).toBe(false)
      if (touches(fit, view, [undo])) {
        // Only where no place at full size clears both: above and below the strip are too low for the picture.
        expect(strip.top - 8 - 64).toBeLessThan(fit.picture * 1.4 + 24)
      }
    },
  )
})

describe('fitClear on a phone', () => {
  const view = { width: 390, height: 844 }

  it('keeps the picture and takes the stretch above the strip, off its buttons', () => {
    const card = rect(25, 300, 95, 400)
    const strip = rect(8, 497, 382, 709)
    const primary = rect(250, 660, 374, 700)
    const clear: ClearBox[] = [
      { ...strip, soft: true },
      { ...primary, kind: 'primary' },
      rect(0, 709, 390, 844),
    ]
    const fit = fitClear(card, view, clear)
    fullSize(fit, card, view)
    expect(touches(fit, view, [primary])).toBe(false)
  })

  it('keeps the picture where the strip fills the window', () => {
    const card = rect(25, 66, 95, 160)
    const fit = fitClear(card, view, [rect(0, 56, 390, 701)])
    fullSize(fit, card, view)
  })
})

describe('fitClear on a short narrow window', () => {
  it('keeps the old picture with the text beside it above the strip', () => {
    const view = { width: 620, height: 460 }
    const card = rect(100, 100, 160, 180)
    const clear = [rect(8, 340, 612, 396), rect(0, 396, 620, 460)]
    const fit = fitClear(card, view, clear)
    fullSize(fit, card, view)
    expect(touches(fit, view, clear)).toBe(false)
  })
})
