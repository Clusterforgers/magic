/** Goal: a scry or surveil of 0 to 5 library cards is drawn as the cards looked at, numbered from the top, in a pile that stays and a pile that goes (the bottom or the graveyard), a click or the card's number moves a card between the piles, the preview and the sentence follow, Confirm sends the cards that go away in the order looked at as a ChooseCards answer exactly once, and a choice of hand cards in the same words stays a plain pick. */
import { screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf } from '@/test-utils/combatTable'
import { renderTable } from '@/test-utils/tableHarness'
import { LOOKED_MOMENTS } from '../dev/lookedPrompts'

const SCRY = 'Put any number of the cards on the bottom of your library.'
const SURVEIL = 'Put any number of the cards into your graveyard.'

const fixture = must(LOOKED_MOMENTS['scry'])(fixtureOf(2))
const library = fixture.view.objects
  .filter((o) => typeof o.zone === 'object' && 'Library' in o.zone)
  .map((o) => o.id)
const source = must(creaturesOf(fixture, 0)[0])
const nameOf = (id: ObjectId): string => must(fixture.view.objects.find((o) => o.id === id)).name
const sourceName = nameOf(source)

function mount(from: ObjectId[], words = SCRY) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    fixture,
    session: { answer },
    state: (state) => ({
      ...state,
      pending: {
        id: 7,
        decision: {
          player: 0,
          kind: { ChooseCards: { source, from, min: 0, max: from.length, text: words } },
        },
      },
    }),
  })
  const root = table.container
  const card = (id: ObjectId): HTMLElement =>
    must(root.querySelector<HTMLElement>(`button[data-candidate="${id}"]`), `card ${id}`)
  const pileOf = (id: ObjectId): string | null =>
    card(id).closest('[data-slot^="look-"]')?.getAttribute('data-slot') ?? null
  const inPile = (slot: 'look-top' | 'look-away'): ObjectId[] =>
    [...must(root.querySelector(`[data-slot="${slot}"]`)).querySelectorAll('button[data-candidate]')].map(
      (b) => Number(b.getAttribute('data-candidate')),
    )
  const text = (): string => must(root.querySelector('[data-slot="prompt-text"]')).textContent ?? ''
  const preview = (): string => must(root.querySelector('[data-slot="look-preview"]')).textContent ?? ''
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm' })
  return { ...table, answer, card, pileOf, inPile, text, preview, confirm, root }
}

describe('the piles at first', () => {
  it.each([1, 2, 3, 5])(
    'hold all %i cards on top, in the order looked at, and Confirm sends none',
    async (count) => {
      const cards = library.slice(0, count)
      const { inPile, card, confirm, user, answer, root } = mount(cards)
      expect(inPile('look-top')).toEqual(cards)
      expect(inPile('look-away')).toEqual([])
      expect(root.querySelector('[data-slot="look-away"] [data-slot="pile-empty"]')).not.toBeNull()
      cards.forEach((id, place) => {
        expect(card(id)).toHaveAttribute('aria-keyshortcuts', String(place + 1))
        expect(card(id)).toHaveAttribute('aria-pressed', 'false')
        expect(card(id)).toHaveAccessibleName(`${nameOf(id)}, on top, card ${place + 1} of ${count}`)
      })
      await user.dblClick(confirm())
      expect(answer).toHaveBeenCalledTimes(1)
      expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseCards: [] })
    },
  )

  it('has focus on the top card, so Space moves it at once', async () => {
    const cards = library.slice(0, 3)
    const { card, user, pileOf } = mount(cards)
    expect(card(must(cards[0]))).toHaveFocus()
    await user.keyboard(' ')
    expect(pileOf(must(cards[0]))).toBe('look-away')
  })

  it('previews the result and tells a screen reader the keys', () => {
    const cards = library.slice(0, 3)
    const { preview, text } = mount(cards)
    expect(preview()).toBe(`On top: ${cards.map(nameOf).join(', ')}. On the bottom: none.`)
    expect(text()).toBe(
      `${sourceName}: ${SCRY} Press 1 to 3 to move that card between the top and the other pile, the top card being 1.`,
    )
  })
})

describe('moving cards', () => {
  // Each case presses the cards at these places (0 is the top card) and expects those places to be sent, in library order.
  it.each([
    ['the top card', [0], [0]],
    ['the third card', [2], [2]],
    ['the last card of five', [4], [4]],
    ['the third then the first, sent in library order', [2, 0], [0, 2]],
    ['a card and back again', [1, 1], []],
    ['every card of five', [4, 3, 2, 1, 0], [0, 1, 2, 3, 4]],
  ])('%s', async (_what, presses, sent) => {
    const cards = library.slice(0, 5)
    const { card, inPile, pileOf, confirm, user, answer } = mount(cards)
    for (const place of presses) await user.click(card(must(cards[place])))
    const away = sent.map((place) => must(cards[place]))
    expect(inPile('look-away')).toEqual(away)
    expect(inPile('look-top')).toEqual(cards.filter((id) => !away.includes(id)))
    for (const id of away) expect(pileOf(id)).toBe('look-away')
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseCards: away })
  })

  it('keeps focus on the card that moved', async () => {
    const cards = library.slice(0, 3)
    const { card, user } = mount(cards)
    await user.click(card(must(cards[1])))
    expect(card(must(cards[1]))).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(card(must(cards[1]))).toHaveFocus()
    expect(card(must(cards[1]))).toHaveAttribute('aria-pressed', 'false')
  })

  it('previews and tells the result after a move', async () => {
    const cards = library.slice(0, 4)
    const { card, user, preview, text } = mount(cards)
    await user.click(card(must(cards[1])))
    await user.click(card(must(cards[3])))
    expect(preview()).toBe(
      `On top: ${nameOf(must(cards[0]))}, ${nameOf(must(cards[2]))}. On the bottom: ${nameOf(must(cards[1]))}, ${nameOf(must(cards[3]))}.`,
    )
    expect(text()).toContain(`${sourceName}: 2 cards stay on top, 2 go to the bottom.`)
    expect(card(must(cards[1]))).toHaveAccessibleName(`${nameOf(must(cards[1]))}, on the bottom, card 2 of 4`)
  })
})

describe('the keys', () => {
  it.each([
    ['1', [0]],
    ['3', [2]],
    ['13', [0, 2]],
    ['11', []],
    ['9', []],
  ])('%s moves the cards at those places', async (keys, sent) => {
    const cards = library.slice(0, 4)
    const { user, confirm, answer } = mount(cards)
    await user.keyboard(keys)
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseCards: sent.map((place) => must(cards[place])) })
  })

  it('leaves focus where it was, and leaves the digits to a text field', async () => {
    const cards = library.slice(0, 3)
    const { card, user, pileOf } = mount(cards)
    await user.keyboard('2')
    expect(card(must(cards[0]))).toHaveFocus()
    const field = document.createElement('input')
    document.body.append(field)
    field.focus()
    await user.keyboard('1')
    field.remove()
    expect(pileOf(must(cards[0]))).toBe('look-top')
  })
})

describe('a surveil', () => {
  it('sends the cards for the graveyard from the graveyard pile', async () => {
    const cards = library.slice(0, 3)
    const { card, user, confirm, answer, text, preview, root } = mount(cards, SURVEIL)
    expect(text()).toContain(SURVEIL)
    await user.click(card(must(cards[2])))
    expect(root.querySelector('[data-slot="look-away"]')).toHaveAccessibleName('In the graveyard: 1')
    expect(preview()).toContain(`In the graveyard: ${nameOf(must(cards[2]))}.`)
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseCards: [cards[2]] })
  })
})

describe('what is not a split', () => {
  it('is a plain pick for cards of a hand asked in the same words', () => {
    const hand = must(fixture.view.players[0]?.hand.cards).slice(0, 3)
    const { root } = mount(hand)
    expect(root.querySelector('[data-slot="look-piles"]')).toBeNull()
    expect(root.querySelector('[data-slot="card-choices"]')).not.toBeNull()
  })

  it('is a plain pick for a search of the library', () => {
    const { root } = mount(library.slice(0, 3), 'Search your library for up to two cards.')
    expect(root.querySelector('[data-slot="look-piles"]')).toBeNull()
  })
})
