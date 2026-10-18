/** Goal: when the engine says the viewer has no attackers or no blockers, the centre strip says so in plain words; the note is about the viewer only, lasts until the viewer has been asked something and answered it or the turn changes, and a later note replaces an earlier one. */
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import { appendLog, initialPlayState } from '@/play/store'
import type { EventView, NothingToDo } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { decisionFor } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { latestNothingToDo, NOTE_TEXT } from './nothingToDo'

const nothing = (player: number, kind: NothingToDo): EventView => ({
  Public: { NothingToDo: { player, kind } },
})
const turn = (number: number, active = 0): EventView => ({ Public: { TurnBegan: { number, active } } })
const step: EventView = { Public: { StepBegan: { step: 'EndCombat' } } }
const passed: EventView = { Public: { PriorityPassed: { player: 0 } } }
const hidden: EventView = { Hidden: { CardDrawn: { player: 0 } } }

const logOf = (...events: EventView[]): PlayState['log'] => appendLog([], 100, events)
const priority = (id: number) => ({ id, decision: decisionFor('Priority') })

describe('latestNothingToDo', () => {
  const state = (log: PlayState['log'], seat: number | null = 0): PlayState => ({
    ...initialPlayState(),
    log,
    seat,
  })

  it.each([
    ['an empty log', [], null],
    ['an attack note', [nothing(0, 'Attack')], { seq: 100, kind: 'Attack' }],
    ['a block note', [step, nothing(0, 'Block'), step], { seq: 101, kind: 'Block' }],
    [
      'the newest of two notes',
      [nothing(0, 'Attack'), step, nothing(0, 'Block')],
      { seq: 102, kind: 'Block' },
    ],
    ['a note about another seat', [nothing(1, 'Attack')], null],
    [
      'a note of another seat after the viewer',
      [nothing(0, 'Attack'), nothing(1, 'Block')],
      { seq: 100, kind: 'Attack' },
    ],
    ['a note before the turn changed', [nothing(0, 'Attack'), turn(6)], null],
    ['a note after the turn changed', [turn(6), step, nothing(0, 'Block')], { seq: 102, kind: 'Block' }],
    ['hidden events around a note', [hidden, nothing(0, 'Attack'), hidden], { seq: 101, kind: 'Attack' }],
    ['no hint of a note', [step, passed], null],
  ] as const)('finds the note in %s', (_name, events, expected) => {
    expect(latestNothingToDo(state(logOf(...events)))).toEqual(expected)
  })

  it('finds nothing when the seat is not known', () => {
    expect(latestNothingToDo(state(logOf(nothing(0, 'Attack')), null))).toBeNull()
  })
})

/** A table with an empty log and no decision, so the test says exactly what has happened. */
function renderQuiet() {
  const table = renderTable({ state: (state) => ({ ...state, log: [], pending: null }) })
  const add = (...events: EventView[]): void =>
    table.update((state) => ({ log: appendLog(state.log, state.log.length, events) }))
  const note = (): HTMLElement | null => table.container.querySelector('[data-slot="nothing-to-do"]')
  return { ...table, add, note }
}

describe('the note in the centre strip', () => {
  it.each([
    ['Attack', 'You have no creatures that can attack'],
    ['Block', 'You have no creatures that can block'],
  ] as const)('says "%s" in plain words', (kind, words) => {
    expect(NOTE_TEXT[kind]).toBe(words)
    const { add, note } = renderQuiet()
    expect(note()).toBeNull()
    add(nothing(0, kind))
    expect(note()).toHaveTextContent(words)
    expect(note()).toHaveAttribute('data-kind', kind)
    expect(screen.getByRole('region', { name: 'Turn and priority' })).toContainElement(note())
  })

  it('sits in a polite live region that is in the page before any note is', () => {
    const { container, add, note } = renderQuiet()
    const region = must(container.querySelector('[aria-live="polite"][data-slot="nothing-to-do-region"]'))
    expect(region).toBeEmptyDOMElement()
    add(nothing(0, 'Attack'))
    expect(region).toContainElement(note())
  })

  it('is about the viewer only', () => {
    const { add, note } = renderQuiet()
    add(nothing(1, 'Attack'))
    expect(note()).toBeNull()
  })

  it('clears when the turn changes', () => {
    const { add, note } = renderQuiet()
    add(nothing(0, 'Attack'))
    expect(note()).not.toBeNull()
    add(turn(6, 1))
    expect(note()).toBeNull()
  })

  it('stays while other things happen in the same turn', () => {
    const { add, note } = renderQuiet()
    add(nothing(0, 'Block'))
    add(step, passed)
    expect(note()).toHaveTextContent('block')
  })

  it('is replaced by a newer note', () => {
    const { add, note } = renderQuiet()
    add(nothing(0, 'Attack'))
    add(step, nothing(0, 'Block'))
    expect(note()).toHaveTextContent('block')
  })

  it('stays while the viewer is asked something, and clears once that is answered', () => {
    const { add, note, update } = renderQuiet()
    add(nothing(0, 'Attack'))
    update(() => ({ pending: priority(1) }))
    expect(note()).not.toBeNull()
    update(() => ({ pending: null }))
    expect(note()).toBeNull()
  })

  it('stays through a decision that is replaced by the next, and clears when the last is answered', () => {
    const { add, note, update } = renderQuiet()
    add(nothing(0, 'Block'))
    update(() => ({ pending: priority(1) }))
    update(() => ({ pending: priority(2) }))
    expect(note()).not.toBeNull()
    update(() => ({ pending: null }))
    expect(note()).toBeNull()
  })

  it('does not come back when the log grows after it was answered', () => {
    const { add, note, update } = renderQuiet()
    add(nothing(0, 'Attack'))
    update(() => ({ pending: priority(1) }))
    update(() => ({ pending: null }))
    add(step, passed)
    expect(note()).toBeNull()
  })

  it('shows a new note after an old one was answered', () => {
    const { add, note, update } = renderQuiet()
    add(nothing(0, 'Attack'))
    update(() => ({ pending: priority(1) }))
    update(() => ({ pending: null }))
    add(nothing(0, 'Block'))
    expect(note()).toHaveTextContent('block')
  })

  it('is shown at once when the viewer is already deciding as it arrives, and goes with that answer', () => {
    const { add, note, update } = renderQuiet()
    update(() => ({ pending: priority(1) }))
    add(nothing(0, 'Attack'))
    expect(note()).not.toBeNull()
    update(() => ({ pending: null }))
    expect(note()).toBeNull()
  })
})
