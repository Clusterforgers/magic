/** Goal: a prompt can write a short note on cards (such as "At Bo"), which reaches only the cards whose note changed, shows on the face and in the spoken name next to the mark, never makes a card clickable, and is gone when the prompt is or the notes are cleared. */
import { act, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { card } from '@/test-utils/cards'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { indexById } from '@/play/view'
import { Card } from '../Card'
import { createMarksStore, MarksContext, NO_NOTES, useCardNotes } from './marks'
import type { CardNote, CardNotes, MarksStore } from './marks'

const note = (label: string, words = label.toLowerCase()): CardNote => ({ label, words })
const notes = (...entries: [number, CardNote][]): CardNotes => new Map(entries)

function watch(store: MarksStore, ids: number[]): Map<number, ReturnType<typeof vi.fn>> {
  const spies = new Map(ids.map((id) => [id, vi.fn()]))
  for (const [id, spy] of spies) store.subscribe(id, spy)
  return spies
}

describe('the notes of a marks store', () => {
  it.each([
    ['a new note', notes(), notes([1, note('At Bo')]), [1]],
    ['a changed label', notes([1, note('At Bo')]), notes([1, note('At Cy')]), [1]],
    ['a changed spoken text', notes([1, note('At Bo', 'a')]), notes([1, note('At Bo', 'b')]), [1]],
    ['a removed note', notes([1, note('At Bo')], [2, note('At Cy')]), notes([2, note('At Cy')]), [1]],
    [
      'the same notes again',
      notes([1, note('At Bo')], [2, note('At Cy')]),
      notes([1, note('At Bo')], [2, note('At Cy')]),
      [],
    ],
    [
      'one of three changing',
      notes([1, note('a')], [2, note('b')], [3, note('c')]),
      notes([1, note('a')], [2, note('B')], [3, note('c')]),
      [2],
    ],
  ] as [string, CardNotes, CardNotes, number[]][])(
    'tells only the cards that changed on %s',
    (_name, before, after, changed) => {
      const store = createMarksStore()
      store.publishNotes(before)
      const spies = watch(store, [1, 2, 3])
      store.publishNotes(after)
      const told = [...spies].filter(([, spy]) => spy.mock.calls.length > 0).map(([id]) => id)
      expect(told).toEqual(changed)
    },
  )

  it('keeps the same note object while a note reads the same, so a card is not redrawn for it', () => {
    const store = createMarksStore()
    store.publishNotes(notes([1, note('At Bo')]))
    const first = store.noteOf(1)
    store.publishNotes(notes([1, note('At Bo')]))
    expect(store.noteOf(1)).toBe(first)
  })

  it('removes every note with clearNotes, and leaves marks alone', () => {
    const store = createMarksStore()
    store.publish(new Map([[1, 'legal']]), undefined, false)
    store.publishNotes(notes([1, note('At Bo')], [2, note('At Cy')]))
    const spies = watch(store, [1, 2])
    store.clearNotes()
    expect([...spies.values()].map((spy) => spy.mock.calls.length)).toEqual([1, 1])
    expect(store.noteOf(1)).toBeUndefined()
    expect(store.markOf(1)).toBe('legal')
  })

  it('keeps NO_NOTES empty', () => {
    expect(NO_NOTES.size).toBe(0)
  })

  it('keeps notes when the marks are cleared', () => {
    const store = createMarksStore()
    store.publish(new Map([[1, 'legal']]), undefined, false)
    store.publishNotes(notes([1, note('At Bo')]))
    store.clear()
    expect(store.noteOf(1)?.label).toBe('At Bo')
  })
})

const ID = 901

/** One card as object 901 in a marks store the test publishes to; `ui` may add a prompt that writes notes. */
function renderNoted(store: MarksStore, extra?: React.ReactNode) {
  return renderTable({
    ui: (
      <MarksContext value={store}>
        <Card id={ID} />
        {extra}
      </MarksContext>
    ),
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, { ...card(), id: ID }]
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
}

const button = (): HTMLElement => screen.getByRole('button')
const face = (): HTMLElement | null => button().querySelector('[data-slot="card-note"]')

describe('a card with a note', () => {
  it('draws the label on its face and speaks the words, after the mark', () => {
    const store = createMarksStore()
    store.publish(new Map([[ID, 'legal']]), undefined, false)
    store.publishNotes(notes([ID, note('At Bo', 'attacking Bo')]))
    renderNoted(store)
    expect(face()).toHaveTextContent('At Bo')
    expect(button().getAttribute('aria-label')).toMatch(/, available, attacking Bo$/)
  })

  it('speaks a note alone, and after a selected mark that reads as pressed', () => {
    const store = createMarksStore()
    store.publishNotes(notes([ID, note('At Bo', 'attacking Bo')]))
    renderNoted(store)
    expect(button().getAttribute('aria-label')).toMatch(/, attacking Bo$/)
    act(() => store.publish(new Map([[ID, 'selected']]), undefined, true))
    expect(button().getAttribute('aria-label')).toMatch(/, attacking Bo$/)
    expect(button()).toHaveAttribute('aria-pressed', 'true')
  })

  it('redraws when its note changes, and loses it when the notes are cleared', () => {
    const store = createMarksStore()
    renderNoted(store)
    expect(face()).toBeNull()
    act(() => store.publishNotes(notes([ID, note('At Bo')])))
    expect(face()).toHaveTextContent('At Bo')
    act(() => store.publishNotes(notes([ID, note('At Cy')])))
    expect(face()).toHaveTextContent('At Cy')
    act(() => store.clearNotes())
    expect(face()).toBeNull()
  })

  it('is not made clickable by a note', async () => {
    const store = createMarksStore()
    const onSelect = vi.fn()
    store.publish(new Map(), onSelect, false)
    store.publishNotes(notes([ID, note('At Bo')]))
    const { user } = renderNoted(store)
    await user.click(button())
    expect(onSelect).not.toHaveBeenCalled()
    expect(button()).not.toHaveAttribute('data-highlight')
    expect(button()).not.toHaveAttribute('aria-pressed')
  })
})

describe('useCardNotes', () => {
  function Writer({ given }: { given: CardNotes }) {
    useCardNotes(given)
    return null
  }

  it('writes the notes while it is on screen, follows changes and clears them when it goes', () => {
    const store = createMarksStore()
    const table = renderNoted(store)
    const view = render(
      <MarksContext value={store}>
        <Writer given={notes([ID, note('At Bo')])} />
      </MarksContext>,
    )
    expect(face()).toHaveTextContent('At Bo')
    view.rerender(
      <MarksContext value={store}>
        <Writer given={notes([ID, note('At Cy')])} />
      </MarksContext>,
    )
    expect(face()).toHaveTextContent('At Cy')
    view.unmount()
    expect(face()).toBeNull()
    table.unmount()
  })
})
