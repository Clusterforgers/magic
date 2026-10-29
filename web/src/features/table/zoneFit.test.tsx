// Goal: a battlefield fits its cards to the room its zone has: without layout the cards keep the sizes they were given; with it each row takes the card width the fit engine answers for the seat's rows (lands at 0.8), a face drops to the short variant under 76 px, a zone that cannot fit at the floor says how many permanents are cut off, a resize fits again, and a zone that scrolls fades the edge that hides more; with stacks on, the rows draw exactly the boxes the fit counts, tapped or not, and the count and the fade still show at the floor.
import { act, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { RowKind } from './cardModel'
import { LAND_SCALE } from './fit'
import type { FitBox } from './fit'
import { battlefieldStacks, layoutSeat } from './rowStacks'
import { fadeOf } from './useEdgeFade'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import type { TableHarness } from '@/test-utils/tableHarness'

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
  fire(): void {
    this.callback([], this as unknown as ResizeObserver)
  }
}

/** The room every element reports, and what the zone holds, until a test changes them. */
const room = { width: 0, height: 0, scroll: 0 }

beforeEach(() => {
  room.width = 0
  room.height = 0
  room.scroll = 0
  vi.stubGlobal('ResizeObserver', FakeObserver)
  vi.spyOn(Element.prototype, 'clientWidth', 'get').mockImplementation(() => room.width)
  vi.spyOn(Element.prototype, 'clientHeight', 'get').mockImplementation(() => room.height)
  vi.spyOn(Element.prototype, 'scrollHeight', 'get').mockImplementation(() => room.scroll)
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  FakeObserver.all.clear()
})

const resized = (): void => {
  act(() => {
    for (const observer of FakeObserver.all) observer.fire()
  })
}

/** What the layout says for a seat's battlefield in a box, rows in the order the zone draws them. */
function expected(table: TableHarness, seat: number, facing: 'near' | 'far', box: FitBox, grouped = false) {
  const stacks = battlefieldStacks(table.store.getState(), seat, grouped)
  const fit = layoutSeat({ box, stacks, facing, opened: new Set(), marked: new Set() }).fit
  if (fit === null) throw new Error('a box was given, so the layout fits')
  const lanes = fit.bands.flatMap((band) => band.lanes)
  const place = (row: RowKind) => must(lanes.find((lane) => lane.lane === row))
  return { fit, order: lanes.map((lane) => lane.lane), place }
}

const rowOf = (seat: number, row: RowKind): HTMLElement => {
  const name = { lands: 'lands', creatures: 'creatures', other: 'other permanents' }[row]
  const owner = seat === 0 ? 'Alice' : 'Bo'
  return screen.getByRole('list', { name: `${owner}'s ${name}` })
}
const widthOf = (element: HTMLElement): string => element.style.getPropertyValue('--card-w')
const heightOf = (element: HTMLElement): string => element.style.getPropertyValue('--card-h')
const sizesIn = (element: HTMLElement): Set<string | null> =>
  new Set([...element.querySelectorAll('[data-size]')].map((card) => card.getAttribute('data-size')))

describe('a battlefield without layout', () => {
  it('keeps the sizes the cards were given and writes no box', () => {
    renderTable({ seats: 2 })
    for (const row of ['creatures', 'other', 'lands'] as const) {
      expect(widthOf(rowOf(1, row))).toBe('')
      expect(heightOf(rowOf(1, row))).toBe('')
    }
    expect(sizesIn(rowOf(1, 'creatures'))).toEqual(new Set(['normal']))
    expect(sizesIn(rowOf(1, 'lands'))).toEqual(new Set(['compact']))
    expect(document.querySelector('[data-slot="more-hidden"]')).toBeNull()
  })
})

describe('a battlefield with room', () => {
  it.each([
    ['a wide, tall zone', { width: 1000, height: 500 }],
    ['a middling zone', { width: 700, height: 240 }],
    ['a small zone', { width: 420, height: 160 }],
  ])('gives each row the card size the fit engine answers in %s', (_name, box) => {
    room.width = box.width
    room.height = box.height
    const table = renderTable({ seats: 2 })
    for (const seat of [0, 1]) {
      const { order, place } = expected(table, seat, seat === 0 ? 'near' : 'far', box)
      order.forEach((row) => {
        const placed = place(row)
        expect(widthOf(rowOf(seat, row)), `${seat} ${row}`).toBe(`${placed.width}px`)
        expect(heightOf(rowOf(seat, row)), `${seat} ${row}`).toBe(`${placed.height}px`)
      })
    }
  })

  it('draws lands at 0.8 of the width of the other rows', () => {
    room.width = 1000
    room.height = 500
    renderTable({ seats: 2 })
    const creatures = Number.parseInt(widthOf(rowOf(1, 'creatures')), 10)
    const lands = Number.parseInt(widthOf(rowOf(1, 'lands')), 10)
    expect(lands).toBe(Math.round(creatures * LAND_SCALE))
  })

  it.each([
    [{ width: 1000, height: 500 }],
    [{ width: 700, height: 240 }],
    [{ width: 420, height: 160 }],
    [{ width: 300, height: 90 }],
  ])('draws the whole face from 76 px and the short one under it in a %o zone', (box) => {
    room.width = box.width
    room.height = box.height
    renderTable({ seats: 2 })
    for (const row of ['creatures', 'other', 'lands'] as const) {
      const wide = Number.parseInt(widthOf(rowOf(1, row)), 10)
      expect(sizesIn(rowOf(1, row)), row).toEqual(new Set([wide >= 76 ? 'normal' : 'compact']))
    }
  })

  it('takes the box from the row, not from the size of the card', () => {
    room.width = 700
    room.height = 240
    renderTable({ seats: 2 })
    const card = rowOf(1, 'creatures').querySelector<HTMLElement>('button[data-object-id]')
    expect(card?.className).toContain('h-(--card-h)')
    // The size classes would set the box again on the card, over the row's.
    expect(card?.className).not.toContain('[--card-w:')
  })

  it('fits again when the zone is resized', () => {
    room.width = 1000
    room.height = 500
    const table = renderTable({ seats: 2 })
    const before = widthOf(rowOf(1, 'creatures'))
    room.width = 420
    room.height = 160
    resized()
    const { place } = expected(table, 1, 'far', { width: 420, height: 160 })
    expect(widthOf(rowOf(1, 'creatures'))).toBe(`${place('creatures').width}px`)
    expect(widthOf(rowOf(1, 'creatures'))).not.toBe(before)
  })

  it('never goes under the 44 px floor and counts the permanents cut off at the bottom', () => {
    room.width = 300
    room.height = 90
    const table = renderTable({ seats: 2 })
    const { fit } = expected(table, 1, 'far', { width: 300, height: 90 })
    expect(fit.scrolls).toBe(true)
    expect(fit.cardWidth).toBe(44)
    expect(widthOf(rowOf(1, 'creatures'))).toBe('44px')
    const pill = document.querySelector('[data-battlefield-seat="1"] [data-slot="more-hidden"]')
    expect(pill).toHaveTextContent(`+${fit.hidden}`)
    expect(fit.hidden).toBeGreaterThan(0)
  })

  it('shows no count when everything fits', () => {
    room.width = 1000
    room.height = 500
    renderTable({ seats: 2 })
    expect(document.querySelector('[data-slot="more-hidden"]')).toBeNull()
  })
})

describe('a battlefield with stacks', () => {
  it.each([
    ['a wide, tall zone', { width: 1000, height: 500 }],
    ['a small zone', { width: 420, height: 160 }],
    ['the floor', { width: 300, height: 90 }],
  ])('draws exactly the boxes the fit counts, tapped or not, in %s', (_name, box) => {
    room.width = box.width
    room.height = box.height
    const table = renderTable({ seats: 2, options: { stacks: true } })
    for (const seat of [0, 1]) {
      const { order, place } = expected(table, seat, seat === 0 ? 'near' : 'far', box, true)
      order.forEach((row) => {
        const placed = place(row)
        const items = rowOf(seat, row).querySelectorAll(':scope > li')
        expect(items, `${seat} ${row}`).toHaveLength(placed.boxes)
        expect(widthOf(rowOf(seat, row)), `${seat} ${row}`).toBe(`${placed.width}px`)
        for (const button of rowOf(seat, row).querySelectorAll('button[data-object-id]')) {
          expect(button).toHaveClass('w-(--card-w)', 'h-(--card-h)')
        }
      })
    }
  })

  it('draws fewer boxes than permanents and so cards at least as large as without stacks', () => {
    room.width = 700
    room.height = 240
    const table = renderTable({ seats: 2, options: { stacks: true } })
    const plain = expected(table, 1, 'far', { width: 700, height: 240 }, false).fit
    const { fit: stacked, place } = expected(table, 1, 'far', { width: 700, height: 240 }, true)
    expect(stacked.cardWidth).toBeGreaterThanOrEqual(plain.cardWidth)
    expect(widthOf(rowOf(1, 'creatures'))).toBe(`${place('creatures').width}px`)
  })

  it('shows the count and the fade at the 44 px floor', () => {
    room.width = 300
    room.height = 90
    room.scroll = 400
    const table = renderTable({ seats: 2, options: { stacks: true } })
    const { fit } = expected(table, 1, 'far', { width: 300, height: 90 }, true)
    expect(fit.scrolls).toBe(true)
    expect(fit.cardWidth).toBe(44)
    expect(document.querySelector('[data-battlefield-seat="1"] [data-slot="more-hidden"]')).toHaveTextContent(
      `+${fit.hidden}`,
    )
    expect(
      document.querySelector('[data-battlefield-seat="1"] [data-slot="battlefield-zone"]'),
    ).toHaveAttribute('data-fade', 'bottom')
  })
})

describe('the edge fade', () => {
  it.each([
    // scrollTop, clientHeight, scrollHeight
    [0, 200, 200, 'none'],
    [0, 200, 201, 'none'],
    [0, 200, 400, 'bottom'],
    [1, 200, 400, 'bottom'],
    [100, 200, 400, 'both'],
    [198, 200, 400, 'both'],
    [199, 200, 400, 'top'],
    [200, 200, 400, 'top'],
    [0, 0, 0, 'none'],
  ] as const)('is %i of %i in %i px: fade the %s', (top, client, total, fade) => {
    expect(fadeOf(top, client, total)).toBe(fade)
  })

  it('marks the zone that holds more than it shows and follows the scrolling', () => {
    room.width = 300
    room.height = 90
    room.scroll = 400
    renderTable({ seats: 2 })
    const zone = document.querySelector<HTMLElement>(
      '[data-battlefield-seat="1"] [data-slot="battlefield-zone"]',
    )
    expect(zone).toHaveAttribute('data-fade', 'bottom')
    zone!.scrollTop = 150
    act(() => {
      zone!.dispatchEvent(new Event('scroll'))
    })
    expect(zone).toHaveAttribute('data-fade', 'both')
    zone!.scrollTop = 310
    act(() => {
      zone!.dispatchEvent(new Event('scroll'))
    })
    expect(zone).toHaveAttribute('data-fade', 'top')
  })
})
