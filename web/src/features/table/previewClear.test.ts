// Goal: the card preview never lies over the open prompt's strip or the docks: at the windows of the table it docks beside the strip or above it at a readable size, narrows to the squeezed picture, else takes a free band above or below a strip that spans the table with its text beside the picture; a phone's takes the free stretch above the strip, and only where nothing is clear does it stand where `previewFit` puts it.
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import type { Anchor, Viewport } from './preview'
import { fitClear, panelBox, READABLE_PICTURE, SQUEEZED_HEIGHT_PX } from './previewClear'
import type { Box, ClearBox } from './previewClear'
import { PICTURE_FLOOR, previewFit } from './previewFit'

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

/** What a Delves table measured with a choice of four cards open and one of them chosen (the strip, the hand dock, the controls). */
const TABLES: [string, Viewport, Anchor, Anchor[]][] = [
  [
    'a wide desktop',
    { width: 1830, height: 855 },
    rect(210, 344, 1486, 488),
    [rect(184, 208, 1518, 550), rect(0, 718, 1006, 855), rect(1006, 718, 1526, 855)],
  ],
  [
    'a laptop',
    { width: 1400, height: 900 },
    rect(210, 373, 1094, 524),
    [rect(184, 237, 1120, 594), rect(0, 756, 859, 900), rect(859, 756, 1128, 900)],
  ],
  [
    'a small laptop',
    { width: 1280, height: 720 },
    rect(210, 316, 968, 416),
    [rect(184, 180, 1000, 468), rect(0, 605, 739, 720), rect(739, 605, 1008, 720)],
  ],
]

describe('fitClear beside a prompt strip', () => {
  it.each(TABLES)('on %s docks beside the strip and covers none of it', (_, view, grid, clear) => {
    const fit = fitClear(grid, view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(touches(previewFit(grid, view), view, clear)).toBe(true)
    expect(fit.dock).toBe('right')
    expect(fit.picture).toBeGreaterThanOrEqual(PICTURE_FLOOR.narrow)
  })

  it.each(TABLES)('on %s leaves the strip alone when the cards sit on its right', (_, view, grid, clear) => {
    // The cards in the right half send the preview to the left, where the sentence and the buttons are.
    const strip = must(clear[0])
    const right = rect(
      strip.left + strip.width * 0.55,
      grid.top,
      strip.left + strip.width - 20,
      grid.top + grid.height,
    )
    const fit = fitClear(right, view, clear)
    expect(previewFit(right, view).dock).toBe('left')
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit.dock).toBe('right')
  })

  it('goes above a strip that spans the window when there is room over it', () => {
    const view = { width: 1600, height: 900 }
    const clear = [rect(100, 520, 1500, 700), rect(0, 760, 1600, 900)]
    const fit = fitClear(rect(600, 560, 1000, 660), view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit.top).toBe(64)
    expect(fit.maxHeight).toBe(520 - 8 - 64)
    expect(fit.picture).toBeGreaterThanOrEqual(PICTURE_FLOOR.narrow)
  })

  it('keeps the panel above the controls of a short window and shows the old picture at least', () => {
    const view = { width: 1280, height: 480 }
    const clear = [rect(0, 376, 1280, 480)]
    const fit = fitClear(rect(100, 200, 200, 340), view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit.maxHeight).toBeGreaterThanOrEqual(SQUEEZED_HEIGHT_PX)
  })

  it('takes a free band of the window when no side beside or above the strip is clear', () => {
    const view = { width: 1024, height: 768 }
    const grid = rect(210, 297, 940, 416)
    const clear = [rect(184, 161, 972, 468)]
    const fit = fitClear(grid, view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit.beside).toBe(true)
  })

  it('is previewFit when nothing is kept clear, and ignores a box without size', () => {
    const view = { width: 1830, height: 855 }
    const grid = rect(210, 344, 1486, 488)
    expect(fitClear(grid, view, [])).toEqual(previewFit(grid, view))
    expect(fitClear(grid, view, [rect(300, 300, 300, 700), rect(0, 0, 0, 0)])).toEqual(previewFit(grid, view))
  })

  it('leaves a panel that already clears the strip as it is', () => {
    const view = { width: 1830, height: 855 }
    const grid = rect(210, 344, 1486, 488)
    expect(fitClear(grid, view, [rect(184, 208, 1400, 550)])).toEqual(previewFit(grid, view))
  })

  it('never touches a strip over a sweep of windows and strips, or falls back to previewFit', () => {
    for (let width = 1024; width <= 1920; width += 128) {
      for (let height = 600; height <= 1080; height += 120) {
        for (const reach of [0.5, 0.6, 0.7]) {
          const strip = rect(184, height * 0.25, Math.round(width * reach) + 300, height * 0.65)
          const grid = rect(210, height * 0.4, strip.left + strip.width - 30, height * 0.55)
          const clear = [strip, rect(0, height - 137, width - 300, height)]
          const view = { width, height }
          const fit = fitClear(grid, view, clear)
          const room = width - 12 - (strip.left + strip.width + 8)
          const touched = touches(fit, view, clear)
          if (room >= PICTURE_FLOOR.narrow + 48) expect(touched).toBe(false)
          if (touched) expect(fit).toEqual(previewFit(grid, view))
        }
      }
    }
  })
})

describe('fitClear on a phone', () => {
  const view = { width: 390, height: 844 }

  it('puts the panel in the stretch above the strip, never over its sentence and buttons', () => {
    const grid = rect(25, 66, 365, 481)
    const clear = [rect(8, 497, 382, 709), rect(0, 709, 230, 844), rect(230, 709, 390, 844)]
    const fit = fitClear(grid, view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(touches(previewFit(grid, view), view, clear)).toBe(true)
    expect(fit).toMatchObject({ dock: 'bottom', bottom: 844 - 489 })
    expect(panelBox(fit, view).top).toBeGreaterThanOrEqual(56)
  })

  it('docks at the top when the cards are low and the strip is above them', () => {
    const grid = rect(25, 500, 365, 640)
    const clear = [rect(8, 60, 382, 300), rect(0, 709, 390, 844)]
    const fit = fitClear(grid, view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit).toMatchObject({ dock: 'top', top: 308 })
  })

  it('stands where previewFit puts it when the strip fills the window', () => {
    const grid = rect(25, 66, 365, 300)
    const clear = [rect(0, 56, 390, 701)]
    expect(fitClear(grid, view, clear)).toEqual(previewFit(grid, view))
  })
})

describe('fitClear on a short narrow window', () => {
  it('keeps the old picture above the strip', () => {
    const view = { width: 620, height: 460 }
    const grid = rect(100, 100, 500, 330)
    const clear = [rect(8, 340, 612, 396), rect(0, 396, 620, 460)]
    const fit = fitClear(grid, view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit).toMatchObject({ picture: PICTURE_FLOOR.narrow, beside: true })
  })
})

describe('fitClear keeps a desktop preview readable', () => {
  /** The strip of a table soft, with its buttons at its right end firm, and the docks. */
  const softTable = (clear: Anchor[]): ClearBox[] => {
    const [strip, ...docks] = clear
    const s = must(strip)
    const actions = rect(
      s.left + s.width - 200,
      s.top + s.height - 44,
      s.left + s.width - 12,
      s.top + s.height - 8,
    )
    const sentence = rect(s.left + 12, s.top + 12, s.left + s.width * 0.5, s.top + 44)
    return [{ ...s, soft: true }, sentence, actions, ...docks]
  }
  const firmOf = (clear: ClearBox[]): Anchor[] => clear.filter((box) => box.soft !== true)

  it.each(TABLES.slice(0, 2))(
    'on %s keeps off the whole strip, its buttons and the docks, at no less than the squeezed picture',
    (_, view, grid, clear) => {
      const kept = softTable(clear)
      const fit = fitClear(grid, view, kept)
      expect(touches(fit, view, kept)).toBe(false)
      expect(fit.picture).toBeGreaterThanOrEqual(PICTURE_FLOOR.narrow)
      expect(touches(fit, view, firmOf(kept))).toBe(false)
    },
  )

  it('docks beside the strip at a readable size where the room is there, covering none of it', () => {
    const view = { width: 1920, height: 900 }
    const grid = rect(210, 344, 1400, 488)
    const clear = softTable([rect(184, 208, 1500, 550), rect(0, 760, 1500, 900)])
    const fit = fitClear(grid, view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit.picture).toBeGreaterThanOrEqual(READABLE_PICTURE)
  })

  it('narrows only where nothing readable fits, and then still keeps off the buttons', () => {
    const [, view, grid, clear] = must(TABLES[2])
    const kept = softTable(clear)
    const fit = fitClear(grid, view, kept)
    expect(touches(fit, view, firmOf(kept))).toBe(false)
    expect(fit.picture).toBeGreaterThanOrEqual(fitClear(grid, view, clear).picture)
  })

  it('keeps off the strip and every firm box over a sweep of windows', () => {
    for (let width = 1024; width <= 2300; width += 128) {
      for (let height = 720; height <= 1080; height += 60) {
        const strip = rect(184, height * 0.3, width - 300, height * 0.6)
        const grid = rect(210, height * 0.4, width - 330, height * 0.55)
        const kept = softTable([strip, rect(0, height - 137, width - 300, height)])
        const view = { width, height }
        const fit = fitClear(grid, view, kept)
        expect(touches(fit, view, firmOf(kept))).toBe(false)
        expect(touches(fit, view, kept)).toBe(false)
      }
    }
  })
})

describe('fitClear beside a card choice with its buttons at the strip start', () => {
  /** A Delves boss table choosing 1 of 3: the sentence and Confirm/Undo at the strip's left, the three cards at its right. */
  const choice = (view: Viewport): { grid: Anchor; clear: ClearBox[]; firm: Anchor[] } => {
    const right = view.width - 312
    const strip = rect(184, 380, right, 600)
    const firm = [
      rect(196, 392, right - 540, 432),
      rect(196, 540, 420, 580),
      rect(0, 718, right - 520, view.height),
      rect(right - 520, 718, right + 8, view.height),
    ]
    return { grid: rect(right - 520, 392, right - 12, 590), clear: [{ ...strip, soft: true }, ...firm], firm }
  }

  it.each([
    [1830, 855],
    [2300, 840],
    [1400, 900],
    [1280, 720],
  ])('at %i x %i never covers the sentence, the buttons or the docks', (width, height) => {
    const view = { width, height }
    const { grid, clear, firm } = choice(view)
    expect(previewFit(grid, view).dock).toBe('left')
    expect(touches(previewFit(grid, view), view, firm)).toBe(true)
    const fit = fitClear(grid, view, clear)
    expect(touches(fit, view, firm)).toBe(false)
    expect(fit.picture).toBeGreaterThanOrEqual(PICTURE_FLOOR.narrow)
  })

  it('keeps at least the floor of a side panel where the window leaves the room, on the side away from the buttons', () => {
    const view = { width: 2300, height: 840 }
    const { grid, clear } = choice(view)
    const fit = fitClear(grid, view, clear)
    expect(fit.dock).toBe('right')
    expect(fit.picture).toBeGreaterThanOrEqual(PICTURE_FLOOR.side)
  })
})

describe('fitClear beside a strip that spans the whole table', () => {
  /** A Delves boss table with a card choice: the strip edge to edge, the chosen card in it, the hand and the controls. */
  const CASES: [string, Viewport, Anchor, Anchor, Anchor[]][] = [
    [
      '1830 x 855, the card at the right',
      { width: 1830, height: 855 },
      rect(8, 380, 1822, 600),
      rect(1500, 400, 1700, 580),
      [rect(0, 718, 1006, 855), rect(1006, 718, 1526, 855)],
    ],
    [
      '2300 x 840, the card at the right',
      { width: 2300, height: 840 },
      rect(8, 330, 2292, 560),
      rect(1900, 350, 2100, 540),
      [rect(0, 705, 1480, 840), rect(1480, 705, 2000, 840)],
    ],
    [
      '1270 x 1380, the card at the centre',
      { width: 1270, height: 1380 },
      rect(8, 245, 1262, 645),
      rect(560, 300, 710, 520),
      [rect(0, 1200, 900, 1380), rect(900, 1200, 1270, 1380)],
    ],
  ]

  it.each(CASES)('on %s lies over neither the strip nor a dock', (_, view, strip, card, docks) => {
    const clear = [strip, ...docks]
    expect(touches(previewFit(card, view), view, clear)).toBe(true)
    const fit = fitClear(card, view, clear)
    expect(touches(fit, view, clear)).toBe(false)
    expect(fit.picture).toBeGreaterThanOrEqual(96)
  })

  it('takes the free band below the strip, over the boards, at a readable size when the band is tall', () => {
    const [, view, strip, card, docks] = must(CASES[2])
    const fit = fitClear(card, view, [strip, ...docks])
    expect(fit).toMatchObject({ beside: true })
    expect(must(fit.top)).toBeGreaterThanOrEqual(645)
    expect(fit.picture).toBeGreaterThanOrEqual(READABLE_PICTURE)
  })

  it('keeps off a soft strip as well while any free band holds the panel', () => {
    const [, view, strip, card, docks] = must(CASES[0])
    const fit = fitClear(card, view, [{ ...strip, soft: true }, ...docks])
    expect(touches(fit, view, [strip, ...docks])).toBe(false)
  })
})
