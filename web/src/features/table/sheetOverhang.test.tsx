// Goal: the prompt sheet lies over no card of the opponents' boards: the table's main area carries how far the sheet stands above the shared strip (0 for a sheet no taller than the strip), it follows the sheet as a prompt grows or shrinks it, it goes with the strip, and the opponents' area keeps that much room free below its boards on top of its own padding. A sheet held at its most asks the split for the room its content wants and scrolls the answers into view.
import { act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { revealBy, SHEET_OVERHANG_VAR, wantedOverhang } from './useSheetOverhang'

/** A `ResizeObserver` the test fires by hand, since jsdom has no layout. */
class FakeObserver {
  static all = new Set<FakeObserver>()
  readonly callback: ResizeObserverCallback
  constructor(callback: ResizeObserverCallback) {
    this.callback = callback
    FakeObserver.all.add(this)
  }
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {
    FakeObserver.all.delete(this)
  }
}

/** The heights the strip and its sheet report, until a test changes them. */
const heights = { strip: 88, sheet: 88 }

beforeEach(() => {
  heights.strip = 88
  heights.sheet = 88
  vi.stubGlobal('ResizeObserver', FakeObserver)
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (this: HTMLElement) {
    if (this.dataset['slot'] === 'shared-sheet') return heights.sheet
    return this.dataset['region'] === 'shared' ? heights.strip : 0
  })
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  FakeObserver.all.clear()
})

const main = (): HTMLElement => must(document.querySelector<HTMLElement>('[data-slot="table-main"]'))
const overhang = (): string => main().style.getPropertyValue(SHEET_OVERHANG_VAR)
const resized = (): void => {
  act(() => {
    for (const observer of FakeObserver.all) observer.callback([], observer as unknown as ResizeObserver)
  })
}

describe('the room kept for the prompt sheet', () => {
  it('is nothing while the sheet is no taller than the strip', () => {
    renderTable({ seats: 4 })
    expect(overhang()).toBe('0px')
  })

  it.each([
    [88, 190, '102px'],
    [65, 112, '47px'],
    [88, 360, '272px'],
  ])("is the sheet's height over the strip's: strip %i px, sheet %i px, room %s", (strip, sheet, room) => {
    heights.strip = strip
    heights.sheet = sheet
    renderTable({ seats: 4 })
    expect(overhang()).toBe(room)
  })

  it('follows a prompt that grows the sheet and one that gives it back', () => {
    renderTable({ seats: 4 })
    heights.sheet = 150
    resized()
    expect(overhang()).toBe('62px')
    heights.sheet = 88
    resized()
    expect(overhang()).toBe('0px')
  })

  it('is left on the main area for the opponents to keep free, below their boards and above the shared strip', () => {
    renderTable({ seats: 4 })
    const opponents = must(document.querySelector('[data-area="opponents"]'))
    expect(main()).toContainElement(opponents as HTMLElement)
    expect(opponents).toHaveClass('pb-[calc(0.5rem+var(--sheet-overhang,0px))]', 'pt-2', 'px-2')
    // The sheet is not in this area, so it can never be drawn over its cards.
    expect(opponents).not.toContainElement(
      document.querySelector('[data-slot="shared-sheet"]') as HTMLElement,
    )
  })

  it('is clipped by each battlefield, so a board squeezed to no room shows no sliver of a card', () => {
    renderTable({ seats: 4 })
    const boards = document.querySelectorAll('[data-area="opponents"] [data-battlefield-seat]')
    expect(boards.length).toBe(3)
    for (const board of boards) expect(board).toHaveClass('overflow-hidden', 'min-h-0')
  })

  it('goes away with the strip', () => {
    heights.sheet = 190
    const table = renderTable({ seats: 4 })
    const grid = main()
    expect(grid.style.getPropertyValue(SHEET_OVERHANG_VAR)).toBe('102px')
    table.unmount()
    expect(grid.style.getPropertyValue(SHEET_OVERHANG_VAR)).toBe('')
  })
})

describe('the overhang a sheet wants', () => {
  it.each([
    // A sheet no taller than the strip with its content in view wants nothing.
    [{ offsetHeight: 56, clientHeight: 52, scrollHeight: 52 }, 56, 0],
    // A sheet grown to its content wants what it stands above the strip.
    [{ offsetHeight: 152, clientHeight: 148, scrollHeight: 148 }, 56, 96],
    // A sheet held at its most still asks for the content scrolled out of it, with its borders.
    [{ offsetHeight: 152, clientHeight: 148, scrollHeight: 200 }, 56, 148],
  ])('of %o over a %i px strip is %i px', (sheet, strip, wanted) => {
    expect(wantedOverhang(sheet, strip)).toBe(wanted)
  })
})

describe('the answers of a sheet held at its most', () => {
  it.each([
    // In view: no scroll.
    [{ top: 120, bottom: 180 }, 0],
    // Below the bottom edge: scrolled up by what is hidden.
    [{ top: 180, bottom: 230 }, 30],
    // Taller than the sheet: its top stays in view.
    [{ top: 150, bottom: 320 }, 50],
    // Above the top edge already: left alone.
    [{ top: 80, bottom: 120 }, 0],
  ])('at %o in a sheet from 100 to 200 px scroll by %i px', (target, by) => {
    expect(revealBy(100, 100, target)).toBe(by)
  })
})
