//! Goal: a pinned card preview never lies over a board's cards: a board it overlaps narrows on that side to keep its cards beside the panel (down to a least width), a board it misses keeps its width, and a hovered preview finds empty room beside the cards at its full size where there is some.
import { act, render, screen } from '@testing-library/react'
import { useRef } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Anchor, Viewport } from './preview'
import { boxOf, fitClear, meets, panelBox } from './previewClear'
import type { Box } from './previewClear'
import { GAP_PX, previewFit } from './previewFit'
import { boardInset, NO_INSET, reservePreview, usePreviewInset, ZONE_KEEP_PX } from './previewReserve'

const box = (left: number, top: number, right: number, bottom: number): Box => ({ left, top, right, bottom })
const rect = (left: number, top: number, right: number, bottom: number): Anchor => ({
  left,
  top,
  width: right - left,
  height: bottom - top,
})

describe('boardInset', () => {
  const zone = box(180, 300, 1560, 860)

  it.each([
    ['a panel at the right', box(1300, 64, 1860, 860), { left: 0, right: 1560 - 1300 + GAP_PX }],
    ['a panel at the left', box(12, 64, 500, 860), { left: 500 - 180 + GAP_PX, right: 0 }],
    ['a panel above the board', box(1300, 64, 1860, 290), NO_INSET],
    ['a panel beside the board', box(1570, 64, 1860, 860), NO_INSET],
    ['no panel', null, NO_INSET],
    ['a panel over nearly all of it', box(200, 64, 1860, 860), { left: 0, right: 1560 - 180 - ZONE_KEEP_PX }],
  ])('insets the board for %s', (_, panel, inset) => {
    expect(boardInset(zone, panel)).toEqual(inset)
  })

  it('keeps every card of the narrowed board out of the panel', () => {
    const panel = box(1300, 64, 1860, 860)
    const inset = boardInset(zone, panel)
    expect(zone.right - inset.right).toBeLessThan(panel.left)
  })
})

function Zone() {
  const ref = useRef<HTMLDivElement>(null)
  const inset = usePreviewInset(ref)
  return <div ref={ref} data-testid="zone" data-left={inset.left} data-right={inset.right} />
}

describe('usePreviewInset', () => {
  afterEach(() => {
    act(() => reservePreview(null))
    vi.restoreAllMocks()
  })

  it('narrows the zone while a preview is reserved over it and widens it again once the preview is gone', () => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(
      DOMRect.fromRect({ x: 180, y: 300, width: 1380, height: 560 }),
    )
    render(<Zone />)
    const zone = screen.getByTestId('zone')
    expect(zone.dataset.right).toBe('0')
    act(() => reservePreview(box(1300, 64, 1860, 860)))
    expect(zone.dataset.right).toBe(String(1560 - 1300 + GAP_PX))
    expect(zone.dataset.left).toBe('0')
    act(() => reservePreview(null))
    expect(zone.dataset.right).toBe('0')
  })
})

describe('fitClear beside the board cards', () => {
  const view: Viewport = { width: 1873, height: 1047 }
  // A creature row at the left of the board under a strip that owes no decision, and the hand dock and controls under it.
  const card = rect(380, 470, 600, 780)
  const clear = [rect(0, 880, 1050, 1047), rect(1050, 880, 1560, 1047)]

  it('stands over empty room at full size where the natural panel would cover cards', () => {
    // The card shown in a row in the middle: the natural right panel lies over the last card, the left one is empty.
    const shown = rect(600, 470, 820, 780)
    const cards = [shown, rect(830, 470, 1050, 780), rect(1400, 470, 1600, 690)]
    const base = previewFit(shown, view)
    expect(cards.some((each) => meets(panelBox(base, view), boxOf(each)))).toBe(true)
    const fit = fitClear(shown, view, clear, cards)
    expect(cards.some((each) => meets(panelBox(fit, view), boxOf(each)))).toBe(false)
    expect(clear.some((each) => meets(panelBox(fit, view), boxOf(each)))).toBe(false)
    expect(fit.picture).toBe(base.picture)
    expect(fit.width).toBe(base.width)
  })

  it('stands as it would without cards where no empty room holds the full size', () => {
    const wall = Array.from({ length: 8 }, (_, at) => rect(184 + at * 210, 64, 384 + at * 210, 870))
    expect(fitClear(card, view, clear, wall)).toEqual(fitClear(card, view, clear))
  })
})
