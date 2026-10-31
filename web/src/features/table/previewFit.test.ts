// Goal: the card preview's fit grows the picture to up to twice its old size as far as the window's height, its width and the room beside the card allow, never below the old size, docks away from the card (flipping at the middle of a narrow window), keeps clear of the card, the hand dock and the controls where the window is large enough, and stays inside the window.
import { describe, expect, it } from 'vitest'
import type { Anchor } from './preview'
import { PICTURE_CEILING, PICTURE_FLOOR, previewFit } from './previewFit'
import type { PreviewFit } from './previewFit'

const box = (left: number, top: number, width = 100, height = 140): Anchor => ({ left, top, width, height })

/** The panel's rectangle in window px. */
function panelOf(fit: PreviewFit, view: { width: number; height: number }) {
  const { dock } = fit
  if (dock === 'left' || dock === 'right') {
    const width = fit.width ?? 0
    const left = dock === 'left' ? 12 : view.width - 12 - width
    return { left, right: left + width, top: fit.top ?? 0, bottom: (fit.top ?? 0) + fit.maxHeight }
  }
  const top = dock === 'top' ? (fit.top ?? 0) : view.height - (fit.bottom ?? 0) - fit.maxHeight
  return { left: 8, right: view.width - 8, top, bottom: top + fit.maxHeight }
}

const overlaps = (panel: ReturnType<typeof panelOf>, card: Anchor): boolean =>
  panel.left < card.left + card.width &&
  panel.right > card.left &&
  panel.top < card.top + card.height &&
  panel.bottom > card.top

describe('previewFit on a wide window', () => {
  it.each([
    // [width, height, picture, panel width, tallest panel]
    [1920, 1080, 480, 528, 867],
    [1440, 900, 384, 432, 711],
    [1366, 768, 340, 388, 596],
    [1280, 720, 308, 356, 552],
    [1024, 768, 336, 384, 596],
  ])(
    'at %ix%i draws the picture %i px wide in a panel %i px wide and at most %i px tall',
    (w, h, picture, width, tall) => {
      const left = previewFit(box(100, 300), { width: w, height: h })
      expect(left).toMatchObject({ dock: 'right', picture, width, maxHeight: tall, top: 64, bottom: null })
      expect(left.beside).toBe(false)
      // The mirror image: a card on the right docks the panel on the left with the same size.
      const right = previewFit(box(w - 200, 300), { width: w, height: h })
      expect(right).toMatchObject({ dock: 'left', picture, width, maxHeight: tall })
    },
  )

  it('draws a picture at least 1.6 times the old one at 1440x900 and twice at 1920x1080', () => {
    expect(previewFit(box(100, 300), { width: 1440, height: 900 }).picture).toBeGreaterThanOrEqual(
      PICTURE_FLOOR.side * 1.6,
    )
    expect(previewFit(box(100, 300), { width: 1920, height: 1080 }).picture).toBe(PICTURE_CEILING.side)
  })

  it('shrinks the picture to leave a wide card uncovered, down to the old size when it cannot', () => {
    const view = { width: 1024, height: 768 }
    const wide = box(300, 300, 400)
    const fit = previewFit(wide, view)
    expect(fit).toMatchObject({ dock: 'right', picture: 256, width: 304 })
    expect(overlaps(panelOf(fit, view), wide)).toBe(false)
    const huge = previewFit(box(200, 300, 600), view)
    expect(huge.picture).toBe(PICTURE_FLOOR.side)
  })

  it('keeps the hand dock and the controls clear while the window is tall enough, and reaches past them when it is not', () => {
    const tall = previewFit(box(100, 300), { width: 1440, height: 900 })
    // The hand dock is 117 px at this height, so the panel ends 8 px above it.
    expect(tall.top === null ? 0 : tall.top + tall.maxHeight).toBeLessThanOrEqual(900 - 117 - 8)
    const short = previewFit(box(100, 100), { width: 844, height: 390 })
    expect(short).toMatchObject({ picture: PICTURE_FLOOR.side, width: 288, maxHeight: 314 })
  })
})

describe('previewFit on a narrow window', () => {
  const phone = { width: 390, height: 844 }

  it.each([
    // [card box, dock, picture, tallest panel]
    [box(20, 100), 'bottom', 249, 469],
    [box(20, 700), 'top', 320, 636],
    // Mid-window the half the card is in is tight: the panel flips to the side with more room.
    [box(280, 352), 'top', 160, 344],
    [box(280, 353), 'top', 160, 344],
  ] as const)(
    'docks the panel for a card at %j on the %s with a picture %i px wide',
    (card, dock, picture, tall) => {
      const fit = previewFit(card, phone)
      expect(fit).toMatchObject({ dock, picture, maxHeight: tall, width: null, beside: false })
      expect(dock === 'top' ? fit.top : fit.bottom).not.toBeNull()
    },
  )

  it('keeps a bottom panel above the hand dock and a top one below the top bar', () => {
    const below = previewFit(box(20, 100), phone)
    expect(below.bottom).toBe(126)
    expect(below.top).toBeNull()
    expect(previewFit(box(20, 700), phone)).toMatchObject({ top: 56, bottom: null })
  })

  it('fits the picture to the window width on a small phone', () => {
    const fit = previewFit(box(20, 700), { width: 340, height: 800 })
    expect(fit.picture).toBe(340 - 16 - 24)
  })

  it('keeps the old size and the text beside the picture in a narrow, short window', () => {
    const fit = previewFit(box(20, 100), { width: 568, height: 320 })
    expect(fit).toMatchObject({
      dock: 'top',
      picture: PICTURE_FLOOR.narrow,
      beside: true,
      top: 56,
      maxHeight: 200,
    })
    expect(previewFit(box(20, 10), { width: 568, height: 320 })).toMatchObject({ dock: 'bottom', bottom: 64 })
  })

  it('switches from the side to a top or bottom panel at 640 px', () => {
    expect(previewFit(box(100, 100), { width: 640, height: 800 }).dock).toBe('right')
    expect(previewFit(box(100, 100), { width: 639, height: 800 }).dock).toBe('bottom')
  })
})

describe('previewFit anywhere', () => {
  const windows = [
    { width: 1920, height: 1080 },
    { width: 1440, height: 900 },
    { width: 1366, height: 768 },
    { width: 1280, height: 720 },
    { width: 1024, height: 768 },
    { width: 640, height: 480 },
    { width: 390, height: 844 },
    { width: 360, height: 640 },
    { width: 568, height: 320 },
  ]
  const spots = [0.02, 0.2, 0.4, 0.5, 0.6, 0.8, 0.95]

  it('stays within the old and the doubled size, inside the window, and covers the card only when it cannot help it', () => {
    for (const view of windows) {
      for (const x of spots) {
        for (const y of spots) {
          const card = box(
            Math.round(view.width * x - 50),
            Math.round(view.height * y - 70),
            Math.min(100, view.width * 0.2),
          )
          const fit = previewFit(card, view)
          const side = view.width >= 640
          const floor = side ? PICTURE_FLOOR.side : PICTURE_FLOOR.narrow
          const ceiling = side ? PICTURE_CEILING.side : PICTURE_CEILING.narrow
          expect(fit.picture).toBeGreaterThanOrEqual(floor)
          expect(fit.picture).toBeLessThanOrEqual(ceiling)
          const panel = panelOf(fit, view)
          expect(panel.top).toBeGreaterThanOrEqual(0)
          expect(panel.bottom).toBeLessThanOrEqual(view.height)
          expect(panel.left).toBeGreaterThanOrEqual(0)
          expect(panel.right).toBeLessThanOrEqual(view.width)
          if (overlaps(panel, card)) expect(fit.picture).toBe(floor)
        }
      }
    }
  })
})
