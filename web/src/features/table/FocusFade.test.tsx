// Goal: in focus mode the strip of tiles leads with the seat whose turn it is, and the large board follows that seat with a cross-fade of 150 ms: the board that leaves stays in the cell, out of reach of the pointer, the keyboard and a screen reader, while the new one fades in, and is gone when the time is up; nothing fades under reduced motion or where the table draws no timed cues; a tile pressed by hand shows that seat's board with the same fade and the turn then leaves it be until Follow the turn is chosen; and an answer is never held back by a fade.
import { act, fireEvent, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { clone, must } from '@/test-utils/batches'
import { primaryButton } from '@/test-utils/passControls'
import { renderTable } from '@/test-utils/tableHarness'
import type { TableHarness, TableSetup } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { FOCUS_FADE_MS } from './useBoardFade'
import type { TableOptions } from './tableOptions'

interface Layer {
  seat: number
  fade: string | null
}

/** The boards in the focus area, the large one first. */
const layers = (): Layer[] =>
  [...document.querySelectorAll<HTMLElement>('[data-slot="focus-layer"]')].map((layer) => ({
    seat: Number(layer.querySelector('[data-seat-area]')?.getAttribute('data-seat-area')),
    fade: layer.getAttribute('data-crossfade'),
  }))
/** The seats of the strip, left to right. */
const strip = (): number[] =>
  [...document.querySelectorAll('[data-tile-seat]')].map((tile) =>
    Number(tile.getAttribute('data-tile-seat')),
  )
const tile = (seat: number): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-tile-seat="${seat}"]`))

/** Six seats in focus mode with the turn cues on; seat 1 has the turn, as the fixture has it. */
function open(options: TableOptions = {}, setup: TableSetup = {}): TableHarness {
  return renderTable({
    fixture: buildFixture({ seats: 6, permanents: 2 }),
    ...setup,
    options: { mode: 'focus', cues: true, ...options },
  })
}

function turnOf(table: TableHarness, seat: number): void {
  table.update((state) => {
    const view = clone(must(state.view))
    view.turn = { ...view.turn, active: seat }
    return { view }
  })
}

const advance = (ms: number): void => {
  act(() => {
    vi.advanceTimersByTime(ms)
  })
}

function reduceMotion(): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    media: query,
    matches: query.includes('reduce'),
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

beforeEach(() => {
  vi.useFakeTimers()
})
afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('the large board that follows the turn', () => {
  it('starts with the board of the active seat and no fade', () => {
    open()
    expect(layers()).toEqual([{ seat: 1, fade: null }])
  })

  it('cross-fades to the seat that gets the turn: both boards for 150 ms, then the new one alone', () => {
    const table = open()
    turnOf(table, 3)
    expect(layers()).toEqual([
      { seat: 3, fade: 'in' },
      { seat: 1, fade: 'out' },
    ])
    advance(FOCUS_FADE_MS - 1)
    expect(layers()).toHaveLength(2)
    advance(1)
    expect(layers()).toEqual([{ seat: 3, fade: null }])
    expect(FOCUS_FADE_MS).toBe(150)
  })

  it('fades once for every change of the seat, in turn order, and never when the same seat stays', () => {
    const table = open()
    for (const seat of [2, 3, 4, 5]) {
      turnOf(table, seat)
      expect(layers().map((layer) => layer.seat)).toEqual([seat, seat - 1])
      advance(FOCUS_FADE_MS)
      expect(layers()).toEqual([{ seat, fade: null }])
    }
    // The same seat getting the turn again changes nothing in the cell.
    turnOf(table, 5)
    expect(layers()).toEqual([{ seat: 5, fade: null }])
  })

  it('keeps the board that leaves out of reach: no pointer, no keyboard, no screen reader', () => {
    const table = open()
    expect(screen.getByRole('region', { name: "Bo's battlefield" })).toBeInTheDocument()
    turnOf(table, 3)
    const leaving = must(document.querySelector<HTMLElement>('[data-crossfade="out"]'))
    expect(leaving).toHaveAttribute('aria-hidden', 'true')
    expect(leaving).toHaveAttribute('inert')
    expect(leaving).toHaveClass('pointer-events-none', 'absolute', 'inset-0')
    expect(screen.queryByRole('region', { name: "Bo's battlefield" })).toBeNull()
    expect(screen.getByRole('region', { name: "Dee's battlefield" })).toBeInTheDocument()
    // The board that arrives takes its place in the flow of the cell, and answers at once.
    const arriving = must(document.querySelector<HTMLElement>('[data-crossfade="in"]'))
    expect(arriving).not.toHaveAttribute('inert')
    expect(arriving).not.toHaveAttribute('aria-hidden')
    expect(arriving).not.toHaveClass('pointer-events-none')
  })

  it('swaps the pod with the board, since the pod is part of what fades', () => {
    const table = open()
    turnOf(table, 3)
    const pods = [
      ...document.querySelectorAll('[data-slot="focus-layer"] [data-slot="pod-column"] section[data-seat]'),
    ]
    expect(pods.map((pod) => pod.getAttribute('data-seat'))).toEqual(['3', '1'])
  })

  it('turns back to a board that was leaving, without a second board of the same seat', () => {
    const table = open()
    turnOf(table, 3)
    advance(FOCUS_FADE_MS / 2)
    turnOf(table, 1)
    expect(layers()).toEqual([
      { seat: 1, fade: 'in' },
      { seat: 3, fade: 'out' },
    ])
    advance(FOCUS_FADE_MS)
    expect(layers()).toEqual([{ seat: 1, fade: null }])
  })

  it('stretches with the slow-down of the dev route, as the effects do', () => {
    const table = open({ effectsSlow: 4 })
    turnOf(table, 3)
    advance(FOCUS_FADE_MS * 4 - 1)
    expect(layers()).toHaveLength(2)
    advance(1)
    expect(layers()).toHaveLength(1)
  })

  it('draws the next board at once under reduced motion', () => {
    reduceMotion()
    const table = open()
    turnOf(table, 3)
    expect(layers()).toEqual([{ seat: 3, fade: null }])
    expect(document.querySelector('[data-crossfade]')).toBeNull()
    expect(screen.getByRole('region', { name: "Dee's battlefield" })).toBeInTheDocument()
  })

  it('draws the next board at once where the table draws no timed cues, as every other test sees it', () => {
    const table = open({ cues: false })
    turnOf(table, 3)
    expect(layers()).toEqual([{ seat: 3, fade: null }])
  })
})

describe('the answers while a board fades', () => {
  it('lets the pass button answer at once, with the answer unchanged', () => {
    const answer = vi.fn(() => true)
    const table = open({}, { session: { answer } })
    turnOf(table, 3)
    expect(layers()).toHaveLength(2)
    fireEvent.click(primaryButton())
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer).toHaveBeenCalledWith({ Priority: 'Pass' })
    expect(layers()).toHaveLength(2)
  })
})

describe('the strip of tiles', () => {
  it('leads with the seat whose turn it is, and keeps the others in turn order', () => {
    const table = open()
    expect(strip()).toEqual([1, 2, 3, 4, 5])
    turnOf(table, 3)
    expect(strip()).toEqual([3, 1, 2, 4, 5])
    turnOf(table, 5)
    expect(strip()).toEqual([5, 1, 2, 3, 4])
  })

  it('goes back to turn order on the viewer’s own turn, which leads nothing', () => {
    const table = open()
    turnOf(table, 4)
    expect(strip()).toEqual([4, 1, 2, 3, 5])
    turnOf(table, 0)
    expect(strip()).toEqual([1, 2, 3, 4, 5])
  })

  it('leads with the active seat on a phone and on a wide window alike', () => {
    for (const viewport of [
      { width: 390, height: 844 },
      { width: 1920, height: 1080 },
    ]) {
      const table = open({ viewport })
      turnOf(table, 4)
      expect(strip(), `${viewport.width} px`).toEqual([4, 1, 2, 3, 5])
      table.unmount()
    }
  })

  it('scrolls back to its start when another seat gets the turn, so the lead is in view', () => {
    const table = open()
    const element = must(document.querySelector<HTMLElement>('[data-slot="tile-strip"]'))
    const scrollTo = vi.fn()
    Object.defineProperties(element, {
      scrollLeft: { value: 180, configurable: true },
      clientWidth: { value: 300, configurable: true },
      scrollTo: { value: scrollTo, configurable: true },
    })
    // Every tile sits inside the visible part, so the tile that is shown asks for no scroll of its own.
    for (const slot of element.querySelectorAll('li')) {
      Object.defineProperties(slot, { offsetLeft: { value: 200 }, offsetWidth: { value: 100 } })
    }
    fireEvent.click(tile(3))
    expect(scrollTo).not.toHaveBeenCalled()
    turnOf(table, 4)
    expect(scrollTo).toHaveBeenCalledExactlyOnceWith({ left: 0 })
  })

  it('marks the active tile and the shown one apart, when they are not the same seat', () => {
    const table = open()
    turnOf(table, 4)
    advance(FOCUS_FADE_MS)
    expect(tile(4)).toHaveAttribute('data-state', 'active')
    expect(tile(4)).toHaveAttribute('aria-current', 'true')
    fireEvent.click(tile(2))
    expect(tile(2)).toHaveAttribute('aria-current', 'true')
    expect(tile(4)).not.toHaveAttribute('aria-current')
    expect(tile(4)).toHaveClass('inset-ring-turn-them')
    expect(tile(2)).not.toHaveClass('inset-ring-turn-them')
    expect(tile(2)).toHaveClass('ring-primary')
  })
})

describe('choosing a seat by hand', () => {
  it('shows the board of a tile that is pressed, with the same fade', () => {
    open()
    fireEvent.click(tile(4))
    expect(layers()).toEqual([
      { seat: 4, fade: 'in' },
      { seat: 1, fade: 'out' },
    ])
    advance(FOCUS_FADE_MS)
    expect(layers()).toEqual([{ seat: 4, fade: null }])
  })

  it('stays on the chosen seat when the turn passes, with no fade, while the strip still leads with the turn', () => {
    const table = open()
    fireEvent.click(tile(2))
    advance(FOCUS_FADE_MS)
    turnOf(table, 5)
    expect(layers()).toEqual([{ seat: 2, fade: null }])
    expect(strip()).toEqual([5, 1, 2, 3, 4])
    expect(tile(2)).toHaveAttribute('aria-current', 'true')
  })

  it('goes back to following the turn, with a fade, when Follow the turn is chosen', async () => {
    vi.useRealTimers()
    const table = open()
    fireEvent.click(tile(2))
    await waitFor(() => expect(layers()).toEqual([{ seat: 2, fade: null }]))
    turnOf(table, 5)
    expect(layers()).toEqual([{ seat: 2, fade: null }])
    await table.user.click(screen.getByRole('button', { name: 'Table menu' }))
    await table.user.click(screen.getByRole('button', { name: 'Follow the turn' }))
    expect(layers()).toEqual([
      { seat: 5, fade: 'in' },
      { seat: 2, fade: 'out' },
    ])
    await waitFor(() => expect(layers()).toEqual([{ seat: 5, fade: null }]))
  })

  it('shows the pressed board at once under reduced motion', () => {
    reduceMotion()
    open()
    fireEvent.click(tile(3))
    expect(layers()).toEqual([{ seat: 3, fade: null }])
  })
})
