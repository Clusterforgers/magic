// Goal: the table is exactly the window high and never scrolls: a 100dvh root that clips, a grid of named regions (top bar, opponents, shared strip, your board, hand dock, controls dock, side area) each able to shrink below its content, the rows and columns the layout code names, and the arrangement of seats, pods and the hand for 2 to 12 seats at the six window sizes of the design.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { gridColumns, gridRows, mainColumns, regionSizes } from './layout'
import type { Viewport } from './layout'

const SIZES: ReadonlyArray<[string, Viewport]> = [
  ['1920x1080', { width: 1920, height: 1080 }],
  ['1440x900', { width: 1440, height: 900 }],
  ['1366x768', { width: 1366, height: 768 }],
  ['1280x720', { width: 1280, height: 720 }],
  ['1024x768', { width: 1024, height: 768 }],
  ['390x844', { width: 390, height: 844 }],
]

const AREAS = ['bar', 'opponents', 'shared', 'own', 'hand', 'controls'] as const

const area = (container: HTMLElement, name: string): HTMLElement =>
  must(container.querySelector<HTMLElement>(`[data-area="${name}"]`), `area ${name}`)
const grid = (container: HTMLElement): HTMLElement =>
  must(container.querySelector<HTMLElement>('[data-slot="table-grid"]'))

describe('the frame', () => {
  it('is exactly the window high, clips, and holds the table in one grid', () => {
    const { container } = renderTable({ seats: 2 })
    const table = screen.getByRole('region', { name: 'Game table' })
    expect(table).toHaveClass('h-dvh', 'overflow-hidden')
    expect(table).toContainElement(grid(container))
    expect(grid(container)).toHaveClass('relative', 'isolate', 'grid', 'h-full', 'min-h-0')
  })

  it('has the six regions in reading order, each a cell that may shrink below its content', () => {
    const { container } = renderTable({ seats: 2 })
    const cells = AREAS.map((name) => area(container, name))
    for (const cell of cells) expect(cell).toHaveClass('min-h-0', 'min-w-0')
    for (let index = 1; index < cells.length; index += 1) {
      const before = must(cells[index - 1])
      const after = must(cells[index])
      expect(before.compareDocumentPosition(after) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    }
  })

  it('names each region in the grid areas, so the hand dock and the controls dock share the bottom row', () => {
    const { container } = renderTable({ seats: 2 })
    expect(area(container, 'bar').style.gridArea).toBe('bar')
    expect(area(container, 'hand').style.gridArea).toBe('hand')
    expect(area(container, 'controls').style.gridArea).toBe('controls')
    const main = must(container.querySelector<HTMLElement>('[data-slot="table-main"]'))
    expect(main.getAttribute('style')).toContain(
      '"bar bar" "opponents opponents" "shared shared" "own own" "hand controls"',
    )
  })

  it.each(SIZES)('writes the rows and columns of the layout code at %s', (_name, viewport) => {
    const { container } = renderTable({ seats: 2, options: { viewport } })
    const regions = regionSizes(viewport)
    const main = must(container.querySelector<HTMLElement>('[data-slot="table-main"]'))
    const style = main.getAttribute('style') ?? ''
    expect(style).toContain(`grid-template-rows: ${gridRows(regions)}`)
    expect(style).toContain(`grid-template-columns: ${mainColumns(regions)}`)
    expect(grid(container).getAttribute('style') ?? '').toContain(
      `grid-template-columns: ${gridColumns(regions.side)}`,
    )
    expect(grid(container)).toHaveAttribute('data-breakpoint', regions.breakpoint)
    expect(grid(container)).toHaveAttribute('data-side', regions.side)
  })

  it('draws nothing but a note while there is no view', () => {
    const { container } = renderTable({ seats: 2, state: (state) => ({ ...state, view: null }) })
    expect(container.querySelector('[data-slot="table-grid"]')).toBeNull()
  })
})

describe('the side area follows the window', () => {
  it.each([
    ['1920x1080', 'column'],
    ['1440x900', 'column'],
    ['1280x720', 'column'],
    ['1024x768', 'rail'],
    ['390x844', 'sheet'],
  ])('is a %s: %s', (name, side) => {
    const viewport = must(SIZES.find(([label]) => label === name)?.[1])
    const { container } = renderTable({ seats: 2, options: { viewport } })
    expect(grid(container)).toHaveAttribute('data-side', side)
    expect(container.querySelectorAll('aside[data-side]')).toHaveLength(side === 'sheet' ? 0 : 1)
  })
})

describe('the seats', () => {
  const panelsIn = (element: HTMLElement): number[] =>
    [...element.querySelectorAll('[data-seat]')].map((each) => Number(each.getAttribute('data-seat')))

  it('draws two seats as one wide board beside a pod column for each seat', () => {
    const { container } = renderTable({ seats: 2, options: { viewport: SIZES[1]?.[1] } })
    expect(container.querySelector('[data-layout]')).toHaveAttribute('data-layout', 'grid')
    expect(panelsIn(area(container, 'opponents'))).toEqual([1])
    expect(panelsIn(area(container, 'own'))).toEqual([0])
    for (const name of ['opponents', 'own']) {
      const column = must(area(container, name).querySelector<HTMLElement>('[data-slot="pod-column"]'))
      expect(column.style.width).toBe('168px')
      expect(column.nextElementSibling).toHaveAttribute('data-battlefield-seat')
    }
  })

  it.each([
    [3, 2],
    [4, 3],
  ])('draws %i seats as %i columns of a one-line pod over a board', (seats, columns) => {
    const { container } = renderTable({ seats, options: { viewport: SIZES[0]?.[1] } })
    const opponents = area(container, 'opponents')
    expect(opponents.querySelector('[data-layout]')).toHaveAttribute('data-columns', String(columns))
    const pods = [...opponents.querySelectorAll('section[data-seat]')]
    expect(pods).toHaveLength(columns)
    for (const pod of pods) expect(pod).toHaveAttribute('data-variant', 'line')
    expect(opponents.querySelector('[data-slot="pod-column"]')).toBeNull()
  })

  it.each([5, 8, 12])('draws %i seats as a strip of tiles and one board', (seats) => {
    const { container } = renderTable({
      fixture: buildFixture({ seats, permanents: 2 }),
      options: { viewport: SIZES[0]?.[1] },
    })
    const opponents = area(container, 'opponents')
    expect(opponents.querySelector('[data-layout]')).toHaveAttribute('data-layout', 'focus')
    expect(within(opponents).getByRole('list', { name: 'Opponents' }).querySelectorAll('li')).toHaveLength(
      seats - 1,
    )
    expect(panelsIn(opponents)).toHaveLength(1)
    expect(opponents.querySelector('[data-slot="pod-column"]')).not.toBeNull()
  })

  it('draws a phone as a strip of tiles, the focused board without a pod, and your own pod as one line', () => {
    const { container } = renderTable({ seats: 2, options: { viewport: SIZES[5]?.[1] } })
    const opponents = area(container, 'opponents')
    expect(opponents.querySelector('[data-layout]')).toHaveAttribute('data-layout', 'focus')
    expect(opponents.querySelectorAll('[data-tile-seat]')).toHaveLength(1)
    expect(opponents.querySelector('[data-seat]')).toBeNull()
    expect(opponents.querySelector('[data-battlefield-seat="1"]')).not.toBeNull()
    const own = must(area(container, 'own').querySelector('section[data-seat="0"]'))
    expect(own).toHaveAttribute('data-variant', 'line')
    expect(own).toHaveTextContent('40')
  })

  it('puts your hand in the hand dock and not in your pod', () => {
    const { container } = renderTable({ seats: 2 })
    const hand = within(area(container, 'hand')).getByRole('list', { name: "Alice's hand" })
    expect(hand.querySelectorAll('button[data-object-id]')).toHaveLength(7)
    expect(area(container, 'own').querySelector('ul[aria-label="Alice\'s hand"]')).toBeNull()
  })

  it('keeps the opponents in turn order after you, whoever you are, in the columns', () => {
    const fixture = buildFixture({ seats: 4, viewer: 2, permanents: 2 })
    const { container } = renderTable({ fixture, options: { viewport: SIZES[0]?.[1] } })
    expect(panelsIn(area(container, 'opponents'))).toEqual([3, 0, 1])
    expect(panelsIn(area(container, 'own'))).toEqual([2])
  })
})

describe('the mode at each window size', () => {
  // Four seats need three boards of 280 px or more side by side.
  it.each([
    ['1920x1080', 'grid'],
    ['1366x768', 'grid'],
    ['1280x720', 'grid'],
    ['1024x768', 'grid'],
    ['390x844', 'focus'],
  ])('opens four seats at %s in %s mode', (name, mode) => {
    const viewport = must(SIZES.find(([label]) => label === name)?.[1])
    const { container } = renderTable({ seats: 4, options: { viewport } })
    expect(container.querySelector('[data-layout]')).toHaveAttribute('data-layout', mode)
  })

  it('opens four seats in focus mode in a window of a tablet, where three columns would be under 280 px', () => {
    const { container } = renderTable({ seats: 4, options: { viewport: { width: 768, height: 1024 } } })
    expect(container.querySelector('[data-layout]')).toHaveAttribute('data-layout', 'focus')
  })
})
