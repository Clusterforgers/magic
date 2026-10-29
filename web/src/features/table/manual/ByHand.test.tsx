// Goal: a card with clauses left to the table shows a mark on its face (picture or text) that a press and the `h` key of the focused card turn into the list of exactly those clauses, in a dialog that takes the focus and gives it back; a card the engine runs whole has no mark, and neither the touch path nor the keyboard path disturbs the card's own press, tab stop or menu.
import { act, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { resetImageChoice, setImageChoice } from '@/features/cards/imageSetting'
import { indexById } from '@/play/view'
import type { ManualClause, ObjectView } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { Card } from '../Card'
import { buildFixture } from '../dev/fixture'
import { setDetailFetcher } from '../printed'
import { ManualMenuProvider } from './ManualMenu'
import { LONG_PRESS_MS } from './useManualMenu'

const ID = 900
const STASIS: ManualClause[] = [
  { text: 'When this Aura enters, draw a card for each tapped creature.', reason: 'not read yet' },
  { text: 'Enchanted creature loses all abilities.', reason: 'not read yet' },
]
const ONE: ManualClause[] = [{ text: 'Flip a coin.', reason: 'a random choice' }]

interface Shown {
  object?: ObjectView
  spectator?: boolean
  onSelect?: (id: number) => void
}

/** One card (object 900) among the few permanents of a small fixture, inside the manual menu's provider. */
function renderHand({ object = card(), spectator, onSelect }: Shown = {}) {
  const table = renderTable({
    fixture: buildFixture({ seats: 2, permanents: 4, hand: 2 }),
    ui: (
      <ManualMenuProvider>
        <Card id={ID} {...(onSelect === undefined ? {} : { highlight: 'legal' as const, onSelect })} />
      </ManualMenuProvider>
    ),
    state: (first) => {
      const view = clone(must(first.view))
      view.objects = [...view.objects, { ...object, id: ID }]
      if (spectator === true) view.spectator = true
      return { ...first, view, objectsById: indexById(view.objects) }
    },
  })
  return table
}

const cardButton = (): HTMLElement =>
  document.querySelector<HTMLElement>(`button[data-object-id="${ID}"]`) as HTMLElement
const mark = (): HTMLElement | null => cardButton().querySelector<HTMLElement>('[data-slot="card-support"]')
const panel = (): HTMLElement => screen.getByRole('dialog')

afterEach(() => {
  vi.useRealTimers()
  resetImageMemory()
  resetImageChoice()
  window.localStorage.clear()
})

describe('which cards show the mark', () => {
  it.each<[string, Partial<ObjectView>, 'chip' | null]>([
    ['a Partial Aura with two clauses', { manual: STASIS, subtypes: ['Aura'] }, 'chip'],
    ['a permanent with one clause', { manual: ONE }, 'chip'],
    ['a card in the hand with a clause', { manual: ONE, zone: { Hand: 0 }, permanent: null }, 'chip'],
    ['a card the engine runs whole', {}, null],
    ['an empty list of clauses', { manual: [] }, null],
    ['a face-down card the seat may not look at', { face_down: true, manual: [], name: '' }, null],
  ])('%s', (_name, over, spot) => {
    renderHand({ object: card(over) })
    expect(mark()?.getAttribute('data-spot') ?? null).toBe(spot)
  })

  it('is a chip of the text face while the picture loads and a mark at the head of the corner once it has', async () => {
    setDetailFetcher(() => Promise.reject(new Error('offline')))
    const { container } = renderHand({ object: card({ manual: STASIS, identity: { oracle_id: 'x' } }) })
    expect(container.querySelectorAll('[data-slot="card-support"]')).toHaveLength(1)
    expect(mark()?.getAttribute('data-spot')).toBe('chip')
    act(() => fireEvent.load(must(container.querySelector('img'))))
    await act(async () => {
      await Promise.resolve()
    })
    expect(container.querySelectorAll('[data-slot="card-support"]')).toHaveLength(1)
    expect(mark()?.getAttribute('data-spot')).toBe('corner')
    expect(mark()).toHaveTextContent('By hand')
    expect(container.querySelector('[data-slot="card-marks-head"]')).toContainElement(mark())
  })

  it('draws it on the text face when pictures are off, and not in the tab order of its own', () => {
    setImageChoice('text')
    renderHand({ object: card({ manual: ONE, identity: { oracle_id: 'x' } }) })
    expect(mark()).not.toBeNull()
    expect(mark()).toHaveAttribute('aria-hidden', 'true')
    expect(cardButton().querySelectorAll('button, [tabindex]').length).toBe(0)
  })

  it('says it in the spoken name of the card, with the key that lists the clauses', () => {
    renderHand({ object: card({ manual: ONE }) })
    expect(cardButton().getAttribute('aria-label')).toContain(
      'some rules are played by hand, press H to read them',
    )
    expect(cardButton()).toHaveAttribute('aria-keyshortcuts', 'h')
  })

  it('leaves the spoken name of a card without clauses as it was', () => {
    renderHand()
    expect(cardButton().getAttribute('aria-label')).not.toContain('by hand')
    expect(cardButton()).not.toHaveAttribute('aria-keyshortcuts')
  })
})

describe('the keyboard path', () => {
  it('lists the clauses on h, takes the focus, and Escape gives it back to the card', async () => {
    const { user } = renderHand({ object: card({ name: 'Observed Stasis', manual: STASIS }) })
    cardButton().focus()
    await user.keyboard('h')
    expect(panel()).toHaveAccessibleName('Observed Stasis: rules played by hand')
    expect(panel()).toHaveFocus()
    const items = within(panel()).getAllByRole('listitem')
    expect(items.map((item) => item.textContent)).toEqual(
      STASIS.map((clause) => `${clause.text} ${clause.reason}`),
    )
    expect(panel()).toHaveAccessibleDescription(/When this Aura enters/)
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(cardButton()).toHaveFocus()
  })

  it('steps from the dialog to Close with Tab and out to the card with the next Tab', async () => {
    const { user } = renderHand({ object: card({ manual: ONE }) })
    cardButton().focus()
    await user.keyboard('h')
    await user.tab()
    expect(within(panel()).getByRole('button', { name: 'Close' })).toHaveFocus()
    await user.tab()
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(cardButton()).toHaveFocus()
  })

  it('closes on Close and on Shift+Tab', async () => {
    const { user } = renderHand({ object: card({ manual: ONE }) })
    cardButton().focus()
    await user.keyboard('h')
    await user.click(within(panel()).getByRole('button', { name: 'Close' }))
    expect(screen.queryByRole('dialog')).toBeNull()
    cardButton().focus()
    await user.keyboard('h')
    await user.tab({ shift: true })
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it.each([
    ['a card without clauses', 'h', card()],
    ['another key', 'j', card({ manual: ONE })],
    ['Ctrl+H', '{Control>}h{/Control}', card({ manual: ONE })],
    ['Shift+H', '{Shift>}h{/Shift}', card({ manual: ONE })],
  ])('opens nothing on %s', async (_name, keys, object) => {
    const { user } = renderHand({ object })
    cardButton().focus()
    await user.keyboard(keys)
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('lists the same clauses as the By hand block of the menu, which m still opens', async () => {
    const { user } = renderHand({ object: card({ manual: STASIS }) })
    cardButton().focus()
    await user.keyboard('h')
    const listed = within(panel())
      .getAllByRole('listitem')
      .map((item) => item.textContent)
    await user.keyboard('{Escape}')
    await user.keyboard('m')
    const block = screen.getByRole('region', { name: 'By hand' })
    expect(
      within(block)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual(listed)
  })

  it('serves a seat that only watches: h lists, m opens no menu', async () => {
    const { user } = renderHand({ object: card({ manual: ONE }), spectator: true })
    cardButton().focus()
    await user.keyboard('m')
    expect(screen.queryByRole('menu')).toBeNull()
    await user.keyboard('h')
    expect(panel()).toHaveTextContent('Flip a coin.')
  })
})

describe('the touch and mouse path', () => {
  it('opens the list on a press of the mark and leaves the press of the card alone', async () => {
    const onSelect = vi.fn()
    const { user } = renderHand({ object: card({ manual: ONE }), onSelect })
    await user.click(must(mark()))
    expect(panel()).toHaveTextContent('Flip a coin.')
    expect(panel()).toHaveTextContent('a random choice')
    expect(onSelect).not.toHaveBeenCalled()
    await user.click(cardButton())
    expect(onSelect).toHaveBeenCalledTimes(1)
  })

  it('is not the long press of the card, which opens the menu', () => {
    vi.useFakeTimers()
    renderHand({ object: card({ manual: ONE }) })
    fireEvent.pointerDown(must(mark()), { pointerType: 'touch', clientX: 20, clientY: 20 })
    act(() => {
      vi.advanceTimersByTime(LONG_PRESS_MS * 2)
    })
    expect(screen.queryByRole('menu')).toBeNull()
  })

  it('closes on a press outside', async () => {
    const { user } = renderHand({ object: card({ manual: ONE }) })
    await user.click(must(mark()))
    expect(panel()).toBeInTheDocument()
    await user.click(document.body)
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('closes when the card leaves the table', async () => {
    const { user, update } = renderHand({ object: card({ manual: ONE }) })
    await user.click(must(mark()))
    update((state) => {
      const view = clone(must(state.view))
      view.objects = view.objects.filter((object) => object.id !== ID)
      return { ...state, view, objectsById: indexById(view.objects) }
    })
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('only shows, with the clauses as its tooltip, where no table holds a menu', () => {
    const { container } = renderTable({
      ui: <Card id={ID} />,
      state: (first) => {
        const view = clone(must(first.view))
        view.objects = [...view.objects, { ...card({ manual: ONE }), id: ID }]
        return { ...first, view, objectsById: indexById(view.objects) }
      },
    })
    const bare = must(container.querySelector<HTMLElement>('[data-slot="card-support"]'))
    expect(bare.getAttribute('title')).toContain('Flip a coin.')
    fireEvent.click(bare)
    expect(screen.queryByRole('dialog')).toBeNull()
  })
})
