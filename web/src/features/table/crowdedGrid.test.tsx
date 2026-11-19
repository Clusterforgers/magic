// Goal: a grid of more opponents than one row of frames holds folds their lands until the viewer chooses, keeps each pod to its name line and one row of figures led by the lands chip and the untapped sources, and leaves your board's lands laid out; one row of frames keeps its lands and wraps its pods where its frames are high enough for a wrapped pod over a row of cards, else keeps them to two lines; the other shapes keep their pods and lands as they were.
import { beforeEach, describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { crowdFoldsLands, gridPodsOneRow, WRAPPED_POD_FRAME_PX } from './boardSplit'
import { LANDS_KEY } from './landPrefs'
import { arrangeSeats } from './layout'

beforeEach(() => {
  window.localStorage.clear()
})

const lanes = (container: HTMLElement, seat: number): Element | null =>
  must(container.querySelector(`[data-battlefield-seat="${seat}"]`)).querySelector('[data-lane="lands"]')

describe('a crowded grid', () => {
  it.each([
    [3, 'grid', false],
    [4, 'grid', false],
    [5, 'grid', true],
    [6, 'grid', true],
    [8, 'grid', true],
    [6, 'focus', false],
    [2, 'grid', false],
  ] as const)("of %i seats in %s mode folds the opponents' lands by default: %s", (seats, mode, crowded) => {
    expect(crowdFoldsLands(arrangeSeats({ seatCount: seats, viewer: 0, mode }))).toBe(crowded)
  })

  it.each([
    [4, null, false],
    [4, 400, false],
    [4, WRAPPED_POD_FRAME_PX, false],
    [4, WRAPPED_POD_FRAME_PX - 1, true],
    [4, 229, true],
    [3, 200, true],
    [6, 900, true],
    [6, null, true],
  ] as const)('of %i seats in frames %s px high keeps its pods to two lines: %s', (seats, height, oneRow) => {
    const plan = arrangeSeats({ seatCount: seats, viewer: 0, mode: 'grid' })
    expect(gridPodsOneRow(plan, plan.boards.length, height)).toBe(oneRow)
  })

  it.each([5, 6, 8])(
    "of %i seats folds every opponent's lands into a chip that leads a one-row pod, yours laid out",
    (seats) => {
      const { container } = renderTable({ seats, options: { foldLands: true, mode: 'grid' } })
      for (let seat = 1; seat < seats; seat += 1) {
        expect(lanes(container, seat)).toBeNull()
        const figures = must(container.querySelector(`[data-seat="${seat}"] [data-slot="pod-figures"]`))
        expect(figures.firstElementChild).toHaveAttribute('data-slot', 'land-summary')
        expect(figures.firstElementChild).toHaveAttribute('aria-expanded', 'false')
      }
      expect(lanes(container, 0)).not.toBeNull()
    },
  )

  it("lays the opponents' lands out once the viewer chose to show lands", () => {
    window.localStorage.setItem(LANDS_KEY, 'show')
    const { container } = renderTable({ seats: 6, options: { foldLands: true, mode: 'grid' } })
    for (const seat of [1, 2, 3, 4, 5]) expect(lanes(container, seat)).not.toBeNull()
  })

  it.each([3, 4])('of one row (%i seats) keeps wrapping pods and its lands laid out', (seats) => {
    const { container } = renderTable({ seats, options: { foldLands: true, mode: 'grid' } })
    expect(container.querySelector('[data-slot="pod-figures"]')).toBeNull()
    for (let seat = 1; seat < seats; seat += 1) expect(lanes(container, seat)).not.toBeNull()
  })
})
