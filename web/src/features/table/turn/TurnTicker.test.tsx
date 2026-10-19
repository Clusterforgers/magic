// Goal: the ticker in the shared strip shows what a batch did in one line (the steps it skipped, the viewer's own reason for a skip and the newest line worth reading), up to three lines of 3 s each, replaced at once by a newer batch, never named for another seat's pass, quiet to a screen reader, keeping its row's height when empty, and absent where the table draws no timed cues.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cueDom, renderCueTable, useCueClock } from '@/test-utils/cueTable'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'
import { begin, grant, hid, pass, passTurn, pub, stepRound } from '@/test-utils/turnEvents'
import { TICKER_LINE_MS } from './timing'
import { TurnTicker } from './TurnTicker'

const ticker = (): HTMLElement | null => document.querySelector('[data-slot="turn-ticker"]')

beforeEach(useCueClock)
afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('the lines of a batch', () => {
  it('shows the skip note first and the newest line next, 3 s each, then empties', () => {
    // On its own, so that no other part of the table's timers count.
    const table = renderCueTable({}, { ui: <TurnTicker /> })
    const idle = vi.getTimerCount()
    table.deliver(passTurn(7, 1), { number: 7, active: 1, step: 'Main1' })
    expect(cueDom().ticker).toBe('Skipped End step, Upkeep, Draw')
    table.advance(TICKER_LINE_MS - 1)
    expect(cueDom().ticker).toBe('Skipped End step, Upkeep, Draw')
    table.advance(1)
    expect(cueDom().ticker).toBe('Bo draws a card')
    table.advance(TICKER_LINE_MS)
    expect(cueDom().ticker).toBeNull()
    // Nothing is left running once the lines are gone (the banner's own run is over by then too).
    table.advance(10_000)
    expect(vi.getTimerCount()).toBe(idle)
  })

  it('shows up to three lines: the skip, the viewer’s own reason and the newest event', () => {
    // Alice's own turn: her combat ends and her second main phase goes by, and the game is in her End step.
    const table = renderCueTable({}, { fixture: buildFixture({ seats: 2, permanents: 2, active: 0 }) })
    const events = [
      pass(0),
      ...stepRound('EndCombat', 0),
      ...stepRound('Main2', 0),
      begin('End'),
      pub({ LandPlayed: { player: 0, object: 1 } }),
      grant(0),
    ]
    table.deliver(events, { step: 'End' })
    const seen: Array<string | null> = [cueDom().ticker]
    table.advance(TICKER_LINE_MS)
    seen.push(cueDom().ticker)
    table.advance(TICKER_LINE_MS)
    seen.push(cueDom().ticker)
    table.advance(TICKER_LINE_MS)
    seen.push(cueDom().ticker)
    expect(seen[0]).toBe('Skipped End of combat, Postcombat main')
    expect(seen[1]).toBe('You had no plays')
    expect(seen[2]).toMatch(/^Alice plays /)
    expect(seen[3]).toBeNull()
  })

  it('shows nothing about another seat’s main phase but the steps that went by', () => {
    const table = renderCueTable()
    const events = [...stepRound('Main1', 1), ...stepRound('BeginCombat', 1), begin('Main2'), grant(1)]
    table.deliver(events, { step: 'Main2' })
    const seen: Array<string | null> = [cueDom().ticker]
    table.advance(TICKER_LINE_MS)
    seen.push(cueDom().ticker)
    expect(seen).toEqual(['Skipped Precombat main, Beginning of combat', null])
  })

  it('is replaced at once by a newer batch, so it is never behind the game', () => {
    const table = renderCueTable()
    table.deliver(passTurn(7, 1), { number: 7, active: 1, step: 'Main1' })
    table.advance(1000)
    table.deliver([hid({ CardDrawn: { player: 1 } })], { step: 'Main1' })
    expect(cueDom().ticker).toBe('Bo draws a card')
    table.advance(TICKER_LINE_MS - 1)
    expect(cueDom().ticker).toBe('Bo draws a card')
    table.advance(1)
    expect(cueDom().ticker).toBeNull()
  })

  it('keeps what it shows when a batch has no line for it', () => {
    const table = renderCueTable()
    table.deliver([hid({ CardDrawn: { player: 1 } })], { step: 'Main1' })
    table.advance(1000)
    table.deliver([pass(1), grant(0)], { step: 'Main1' })
    expect(cueDom().ticker).toBe('Bo draws a card')
    table.advance(TICKER_LINE_MS - 1000)
    expect(cueDom().ticker).toBeNull()
  })

  it('stretches with the dev slow-down', () => {
    const table = renderCueTable({ effectsSlow: 2 })
    table.deliver([hid({ CardDrawn: { player: 1 } })])
    table.advance(TICKER_LINE_MS * 2 - 1)
    expect(cueDom().ticker).toBe('Bo draws a card')
    table.advance(1)
    expect(cueDom().ticker).toBeNull()
  })

  it('shows the same lines for the same time under reduced motion, which only the stylesheet changes', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      media: query,
      matches: query.includes('reduce'),
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }))
    const table = renderCueTable()
    table.deliver(passTurn(7, 1), { number: 7, active: 1, step: 'Main1' })
    expect(cueDom().ticker).toBe('Skipped End step, Upkeep, Draw')
    table.advance(TICKER_LINE_MS)
    expect(cueDom().ticker).toBe('Bo draws a card')
  })
})

describe('the line as part of the strip', () => {
  it('keeps its row’s height with nothing to say, so the strip does not move', () => {
    const table = renderCueTable()
    expect(ticker()).toHaveClass('h-4')
    table.deliver(passTurn(7, 1), { number: 7, active: 1, step: 'Main1' })
    expect(ticker()).toHaveClass('h-4')
    expect(ticker()).toHaveClass('truncate')
  })

  it('sits in the strip’s sheet, below what the strip holds', () => {
    renderCueTable()
    const sheet = document.querySelector('[data-slot="shared-sheet"]')
    expect(sheet?.contains(ticker())).toBe(true)
    expect(sheet?.lastElementChild?.lastElementChild).toBe(ticker())
  })

  it('is for the eye: not a live region, hidden from a screen reader, and nothing in it can be pressed', () => {
    const table = renderCueTable()
    table.deliver(passTurn(7, 1), { number: 7, active: 1, step: 'Main1' })
    expect(ticker()).toHaveAttribute('aria-hidden', 'true')
    expect(ticker()).not.toHaveAttribute('aria-live')
    expect(ticker()?.querySelector('button, a, input, [tabindex]')).toBeNull()
  })

  it('says nothing that tells how another seat passed', () => {
    const table = renderCueTable()
    const texts: string[] = []
    const take = () => {
      const text = cueDom().ticker
      if (text !== null) texts.push(text)
    }
    table.deliver(passTurn(7, 1), { number: 7, active: 1, step: 'Main1' })
    for (let step = 0; step < 3; step += 1) {
      take()
      table.advance(TICKER_LINE_MS)
    }
    table.deliver([pass(1), ...stepRound('BeginCombat', 1)], { step: 'BeginCombat' })
    take()
    expect(texts.length).toBeGreaterThan(1)
    for (const text of texts)
      expect(text).not.toMatch(/auto|no plays|nothing to|deciding|thinking|waiting|second/i)
  })

  it('is not in the page where the table draws no timed cues', () => {
    vi.useRealTimers()
    renderTable({ seats: 2 })
    expect(ticker()).toBeNull()
  })
})
