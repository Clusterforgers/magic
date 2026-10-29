// Goal: the turn banner shows "Your turn" or "Bo's turn" with the turn number only for a batch that changes the turn or the active seat, fades in for 250 ms, is held for 600 and fades out for 150 (a fade only, held 1.5 s, under reduced motion), is announced through a polite live region, never takes the place of an answer, and is not drawn at all where the table draws no timed cues.
import { fireEvent } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cueDom, renderCueTable, useCueClock } from '@/test-utils/cueTable'
import { renderTable } from '@/test-utils/tableHarness'
import { grant, pass, passTurn, stepRound } from '@/test-utils/turnEvents'
import { BANNER_TIMES, BANNER_TIMES_REDUCED } from './timing'
import { TurnBanner } from './TurnBanner'

const banner = (): HTMLElement | null => document.querySelector('[data-slot="turn-banner"]')

beforeEach(useCueClock)
afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

function reduceMotion(): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    media: query,
    matches: query.includes('reduce'),
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

describe('what makes the banner show', () => {
  it('waits, drawn but not shown, until the turn changes', () => {
    renderCueTable()
    expect(cueDom()).toMatchObject({ phase: 'idle', announcement: '' })
    expect(banner()).toHaveAttribute('aria-hidden', 'true')
  })

  it("says Bo's turn and its number when the turn passes to another seat", () => {
    const table = renderCueTable()
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    table.deliver(passTurn(7, 1), { number: 7, active: 1, step: 'Main1' })
    expect(cueDom()).toMatchObject({ phase: 'in', banner: "Bo's turn" + 'Turn 7' })
    expect(banner()).toHaveClass('bg-turn-them', 'text-on-turn-them')
    expect(banner()).not.toHaveAttribute('data-your-turn')
  })

  it('says Your turn in the colour of your own turn', () => {
    const table = renderCueTable()
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    expect(cueDom()).toMatchObject({ phase: 'in', banner: 'Your turn' + 'Turn 6' })
    expect(banner()).toHaveClass('bg-turn-you', 'text-on-turn-you')
    expect(banner()).toHaveAttribute('data-your-turn', 'true')
  })

  it('shows for a new active seat in the same turn, and not for a batch inside a turn', () => {
    const table = renderCueTable()
    table.deliver([pass(1), grant(0)], { active: 1, step: 'Main2' })
    expect(cueDom().phase).toBe('idle')
    table.deliver([pass(1)], { active: 0 })
    expect(cueDom()).toMatchObject({ phase: 'in', banner: 'Your turn' + 'Turn 5' })
  })

  it('stays away for a snapshot, which moves the turn without a batch', () => {
    const table = renderCueTable()
    table.update((state) => {
      const view = structuredClone(state.view)
      if (view === null) return {}
      view.turn = { ...view.turn, number: 9, active: 0 }
      return { view, seq: (state.seq ?? 0) + 40 }
    })
    expect(cueDom().phase).toBe('idle')
  })

  it('stays away when the game is not in its turns', () => {
    const table = renderCueTable()
    table.update((state) => ({ view: state.view && { ...state.view, stage: 'Finished' } }))
    table.deliver(stepRound('End', 1), { number: 6, active: 0 })
    expect(cueDom().phase).toBe('idle')
  })
})

describe('its timing', () => {
  it('fades in for 250 ms, is held for 600 and fades out for 150, then goes', () => {
    // On its own, so that no other part of the table's timers count.
    const table = renderCueTable({}, { ui: <TurnBanner /> })
    const idle = vi.getTimerCount()
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    expect(cueDom().phase).toBe('in')
    table.advance(BANNER_TIMES.inMs - 1)
    expect(cueDom().phase).toBe('in')
    table.advance(1)
    expect(cueDom().phase).toBe('hold')
    table.advance(BANNER_TIMES.holdMs - 1)
    expect(cueDom().phase).toBe('hold')
    table.advance(1)
    expect(cueDom().phase).toBe('out')
    table.advance(BANNER_TIMES.outMs - 1)
    expect(cueDom().phase).toBe('out')
    table.advance(1)
    expect(cueDom().phase).toBe('idle')
    // It leaves no timer behind once it is gone.
    table.advance(10_000)
    expect(vi.getTimerCount()).toBe(idle)
  })

  it('starts again from the fade in when another turn begins while it shows', () => {
    const table = renderCueTable()
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    table.advance(BANNER_TIMES.inMs + 100)
    expect(cueDom().phase).toBe('hold')
    table.deliver(passTurn(7, 1), { number: 7, active: 1, step: 'Main1' })
    expect(cueDom()).toMatchObject({ phase: 'in', banner: "Bo's turn" + 'Turn 7' })
    table.advance(BANNER_TIMES.inMs)
    expect(cueDom().phase).toBe('hold')
  })

  it('stretches with the dev slow-down', () => {
    const table = renderCueTable({ effectsSlow: 4 })
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    table.advance(BANNER_TIMES.inMs * 4 - 1)
    expect(cueDom().phase).toBe('in')
    table.advance(1)
    expect(cueDom().phase).toBe('hold')
  })

  it('only fades, and is held for 1.5 s, under reduced motion', () => {
    reduceMotion()
    const table = renderCueTable()
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    table.advance(BANNER_TIMES_REDUCED.inMs)
    expect(cueDom().phase).toBe('hold')
    table.advance(BANNER_TIMES_REDUCED.holdMs - 1)
    expect(cueDom().phase).toBe('hold')
    table.advance(1)
    expect(cueDom().phase).toBe('out')
    table.advance(BANNER_TIMES_REDUCED.outMs)
    expect(cueDom().phase).toBe('idle')
    expect(BANNER_TIMES_REDUCED.holdMs).toBe(1500)
  })
})

describe('what it does for a reader and for an answer', () => {
  it('is announced by a polite live region, with the steps skipped, and the band itself is not read', () => {
    const table = renderCueTable()
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    const region = document.querySelector('[data-slot="turn-announcement"]')
    expect(region).toHaveAttribute('aria-live', 'polite')
    expect(region).toHaveAttribute('aria-atomic', 'true')
    expect(region).toHaveTextContent('Your turn, turn 6. Skipped End step, Upkeep, Draw')
    expect(banner()).toHaveAttribute('aria-hidden', 'true')
  })

  it('leaves the pass button answering while it shows, and its answer unchanged', () => {
    const answer = vi.fn(() => true)
    const table = renderCueTable({}, { session: { answer } })
    table.deliver(passTurn(6, 0), { number: 6, active: 0, step: 'Main1' })
    expect(cueDom().phase).toBe('in')
    const primary = document.querySelector<HTMLElement>('[data-slot="primary-action"]')
    expect(primary).not.toBeNull()
    fireEvent.click(primary as HTMLElement)
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer).toHaveBeenCalledWith({ Priority: 'Pass' })
    expect(cueDom().phase).toBe('in')
  })

  it('is not in the page where the table draws no timed cues, which is every test by default', () => {
    vi.useRealTimers()
    renderTable({ seats: 2 })
    expect(banner()).toBeNull()
    expect(document.querySelector('[data-slot="turn-announcement"]')).toBeNull()
  })
})

describe('where it sits', () => {
  it('hangs on the top edge of the strip when the sheet cannot be measured, and covers nothing', () => {
    renderCueTable()
    expect(banner()).toHaveClass('absolute', 'inset-x-0', 'bottom-full')
    expect(banner()).not.toHaveClass('inset-0')
  })

  it.each([88, 190, 360])('hangs 4 px above a sheet of %i px, however tall a prompt makes it', (height) => {
    vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(height)
    renderCueTable()
    expect(banner()?.style.bottom).toBe(`${height + 4}px`)
    expect(banner()).not.toHaveClass('bottom-full')
  })
})
