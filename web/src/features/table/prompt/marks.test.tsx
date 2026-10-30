/** Goal: a prompt marks cards through a store that tells only the cards whose mark changed, and a Card acts on a mark (look, spoken name, click) unless its own props say otherwise. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Mock } from 'vitest'
import { card } from '@/test-utils/cards'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { indexById } from '@/play/view'
import type { Highlight } from '../cardModel'
import { Card } from '../Card'
import type { CardProps } from '../Card'
import { createMarksStore, markCards, MarksContext, NO_MARKS } from './marks'
import type { CardMarks, MarksStore } from './marks'

const marks = (...entries: [number, Highlight][]): CardMarks => new Map(entries)

describe('markCards', () => {
  it.each([
    [
      'nothing is dimmed and nothing legal',
      [1, 2],
      [],
      [],
      [
        [1, 'dimmed'],
        [2, 'dimmed'],
      ],
    ],
    [
      'legal cards win over dimmed ones',
      [1, 2, 3],
      [2],
      [],
      [
        [1, 'dimmed'],
        [2, 'legal'],
        [3, 'dimmed'],
      ],
    ],
    [
      'selected wins over legal',
      [1, 2],
      [1, 2],
      [2],
      [
        [1, 'legal'],
        [2, 'selected'],
      ],
    ],
    [
      'a legal card outside the own cards is still marked',
      [1],
      [9],
      [],
      [
        [1, 'dimmed'],
        [9, 'legal'],
      ],
    ],
    ['no own cards and no legal ones is empty', [], [], [], []],
  ] as const)('%s', (_name, own, legal, selected, expected) => {
    expect([...markCards(own, legal, selected)]).toEqual(expected)
  })
})

describe('the marks store', () => {
  /** Subscribes one spy per id and returns them. */
  function watch(store: MarksStore, ids: number[]): Map<number, Mock> {
    const spies = new Map<number, Mock>()
    for (const id of ids) {
      const spy = vi.fn()
      store.subscribe(id, spy)
      spies.set(id, spy)
    }
    return spies
  }

  it('starts empty', () => {
    const store = createMarksStore()
    expect(store.markOf(1)).toBeUndefined()
    expect(store.toggles()).toBe(false)
  })

  it.each([
    ['a new mark', marks(), marks([1, 'legal']), [1]],
    ['a changed mark', marks([1, 'legal'], [2, 'dimmed']), marks([1, 'selected'], [2, 'dimmed']), [1]],
    ['a removed mark', marks([1, 'legal'], [2, 'dimmed']), marks([2, 'dimmed']), [1]],
    ['the same marks again', marks([1, 'legal'], [2, 'dimmed']), marks([1, 'legal'], [2, 'dimmed']), []],
    [
      'every card changing',
      marks([1, 'legal'], [2, 'legal'], [3, 'legal']),
      marks([1, 'dimmed'], [2, 'dimmed'], [3, 'dimmed']),
      [1, 2, 3],
    ],
  ] as const)('tells only the cards that changed on %s', (_name, before, after, changed) => {
    const store = createMarksStore()
    store.publish(before, undefined, false)
    const spies = watch(store, [1, 2, 3])
    store.publish(after, undefined, false)
    const told = [...spies].filter(([, spy]) => spy.mock.calls.length > 0).map(([id]) => id)
    expect(told).toEqual(changed)
  })

  it('tells the marked cards, and no other, when marks start to act as toggles, and not before', () => {
    const store = createMarksStore()
    store.publish(marks([1, 'legal']), undefined, false)
    const spies = watch(store, [1, 2])
    store.publish(marks([1, 'legal']), undefined, false)
    expect([...spies.values()].map((spy) => spy.mock.calls.length)).toEqual([0, 0])
    store.publish(marks([1, 'legal']), undefined, true)
    expect([...spies.values()].map((spy) => spy.mock.calls.length)).toEqual([1, 0])
    expect(store.toggles()).toBe(true)
  })

  it('tells the cards marked before and after when toggling flips together with the marks', () => {
    const store = createMarksStore()
    store.publish(marks([1, 'legal']), undefined, false)
    const spies = watch(store, [1, 2, 3])
    store.publish(marks([2, 'dimmed']), undefined, true)
    expect([...spies.values()].map((spy) => spy.mock.calls.length)).toEqual([1, 1, 0])
    store.clear()
    expect([...spies.values()].map((spy) => spy.mock.calls.length)).toEqual([1, 2, 0])
  })

  it('forwards select to the latest handler and to nobody after clear', () => {
    const store = createMarksStore()
    const first = vi.fn()
    const second = vi.fn()
    store.publish(marks([1, 'legal']), first, false)
    store.publish(marks([1, 'legal']), second, false)
    const element = document.createElement('button')
    store.select(1, element)
    expect(first).not.toHaveBeenCalled()
    expect(second).toHaveBeenCalledWith(1, element)
    store.clear()
    store.select(1)
    expect(second).toHaveBeenCalledTimes(1)
    expect(store.markOf(1)).toBeUndefined()
  })

  it('stops telling a card that unsubscribed', () => {
    const store = createMarksStore()
    const spy = vi.fn()
    const off = store.subscribe(1, spy)
    off()
    store.publish(marks([1, 'legal']), undefined, false)
    expect(spy).not.toHaveBeenCalled()
  })

  it('keeps NO_MARKS empty', () => {
    expect(NO_MARKS.size).toBe(0)
  })
})

const ID = 900

interface Shown {
  props?: Partial<CardProps>
  published?: { marks: CardMarks; toggles?: boolean }
  onSelect?: (id: number) => void
}

/** One card in the store as object 900, inside a marks store the test publishes to. */
function renderMarked({ props, published, onSelect }: Shown) {
  const store = createMarksStore()
  if (published !== undefined) store.publish(published.marks, onSelect, published.toggles ?? false)
  const table = renderTable({
    ui: (
      <MarksContext value={store}>
        <Card id={ID} {...props} />
      </MarksContext>
    ),
    state: (state) => {
      const view = clone(must(state.view))
      view.objects = [...view.objects, { ...card(), id: ID }]
      return { ...state, view, objectsById: indexById(view.objects) }
    },
  })
  return { ...table, marks: store }
}

const button = (): HTMLElement => screen.getByRole('button')

describe('a card under a prompt', () => {
  it.each([
    ['legal', false, 'available', undefined],
    ['dimmed', false, 'not available', undefined],
    ['selected', false, 'selected', undefined],
    ['legal', true, 'available', 'false'],
    ['selected', true, undefined, 'true'],
  ] as const)(
    'marked %s (toggles: %s) says so in its name and pressed state',
    (highlight, toggles, word, pressed) => {
      renderMarked({ published: { marks: marks([ID, highlight]), toggles } })
      expect(button()).toHaveAttribute('data-highlight', highlight)
      if (word === undefined) expect(button().getAttribute('aria-label')).not.toMatch(/available|selected$/)
      else expect(button().getAttribute('aria-label')).toMatch(new RegExp(`, ${word}$`))
      if (pressed === undefined) expect(button()).not.toHaveAttribute('aria-pressed')
      else expect(button()).toHaveAttribute('aria-pressed', pressed)
    },
  )

  it.each([
    ['legal', true],
    ['selected', true],
    ['dimmed', false],
  ] as const)('a %s card %s the prompt of a click', async (highlight, selects) => {
    const onSelect = vi.fn()
    const { user } = renderMarked({ published: { marks: marks([ID, highlight]) }, onSelect })
    await user.click(button())
    expect(onSelect.mock.calls).toEqual(selects ? [[ID, button()]] : [])
  })

  it('answers the keyboard like a click', async () => {
    const onSelect = vi.fn()
    const { user } = renderMarked({ published: { marks: marks([ID, 'legal']) }, onSelect })
    await user.tab()
    await user.keyboard('{Enter}')
    await user.keyboard(' ')
    expect(onSelect).toHaveBeenCalledTimes(2)
  })

  it('has no mark and no action without a prompt', async () => {
    const onSelect = vi.fn()
    const { user } = renderMarked({ onSelect })
    await user.click(button())
    expect(button()).not.toHaveAttribute('data-highlight')
    expect(onSelect).not.toHaveBeenCalled()
  })

  it('redraws its mark when the prompt changes it, and loses it on clear', () => {
    const { marks: store } = renderMarked({ published: { marks: marks([ID, 'legal']) } })
    expect(button()).toHaveAttribute('data-highlight', 'legal')
    act(() => store.publish(marks([ID, 'dimmed']), undefined, false))
    expect(button()).toHaveAttribute('data-highlight', 'dimmed')
    act(() => store.clear())
    expect(button()).not.toHaveAttribute('data-highlight')
  })

  it('lets its own props win over the prompt', async () => {
    const mine = vi.fn()
    const theirs = vi.fn()
    const { user } = renderMarked({
      props: { highlight: 'selected', onSelect: mine },
      published: { marks: marks([ID, 'legal']) },
      onSelect: theirs,
    })
    expect(button()).toHaveAttribute('data-highlight', 'selected')
    await user.click(button())
    expect(mine).toHaveBeenCalledWith(ID, button())
    expect(theirs).not.toHaveBeenCalled()
  })
})
