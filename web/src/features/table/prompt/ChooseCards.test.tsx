/** Goal: for choices of exactly one, of several, of up to some and of at least one card, from a hand, the battlefield and a long list, the prompt asks in the card's words (a sentence from min and max without them), draws the cards on offer from the views, chooses by click and by number key under the rules of one or several, keeps Confirm closed until the number is legal and sends the cards in the order chosen once, marks the table's cards on offer as targets are marked, filters a long list without losing the choice, moves focus along the cards with the arrow keys, and leaves the keys to a text field. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, ownOf, renderDecision } from '@/test-utils/combatTable'

const fixture = fixtureOf(2)
const hand = must(fixture.view.players[0]?.hand.cards)
const mine = creaturesOf(fixture, 0).slice(0, 4)
const source = must(creaturesOf(fixture, 1)[0])
const sourceName = must(fixture.view.objects.find((object) => object.id === source)).name
const nameOf = (id: ObjectId): string => must(fixture.view.objects.find((object) => object.id === id)).name
/** Seat 0's permanents on the battlefield: a list long enough to get a filter box. */
const permanents = fixture.view.objects
  .filter((object) => object.zone === 'Battlefield' && object.controller === 0)
  .map((object) => object.id)

function mount(from: ObjectId[], min: number, max: number, text?: string) {
  const table = renderDecision({
    ChooseCards: { source, from, min, max, ...(text === undefined ? {} : { text }) },
  })
  const group = (): HTMLElement => screen.getByRole('group', { name: 'Cards to choose from' })
  const cards = (): HTMLElement[] => within(group()).queryAllByRole('button')
  const card = (place: number): HTMLElement => must(cards()[place])
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm' })
  const count = (): string =>
    must(table.container.querySelector('[data-slot="cards-count"]')).textContent ?? ''
  const chosen = (): ObjectId[] =>
    cards()
      .filter((c) => c.getAttribute('aria-pressed') === 'true')
      .map((c) => Number(c.getAttribute('data-candidate')))
  const sent = (): unknown => table.answer.mock.calls[0]?.[0]
  /** The table's own card of an object, which is not the prompt's. */
  const tableCard = (id: ObjectId): HTMLElement => table.card(id)
  return { ...table, group, cards, card, confirm, count, chosen, sent, tableCard }
}

describe('the words', () => {
  it.each([
    ['Discard two cards.', 2, 2, 7, `${sourceName}: Discard two cards.`, 'Choose 2 of 7'],
    ['Sacrifice a creature.', 1, 1, 4, `${sourceName}: Sacrifice a creature.`, 'Choose 1 of 4'],
    [
      'Search for up to two cards.',
      0,
      2,
      7,
      `${sourceName}: Search for up to two cards.`,
      'Choose up to 2 of 7',
    ],
  ])('"%s" reads with the asker and counts "%s"', (words, min, max, total, sentence, rule) => {
    const { text, count } = mount(hand.slice(0, total), min, max, words)
    expect(text()).toContain(sentence)
    expect(count()).toBe(`${rule} · 0 chosen`)
  })

  it.each([
    [1, 1, 'Choose a card.'],
    [2, 2, 'Choose two cards.'],
    [0, 2, 'Choose up to two cards.'],
    [1, 7, 'Choose at least one card.'],
  ])('without words min %i and max %i read "%s"', (min, max, title) => {
    expect(mount(hand, min, max).text()).toContain(`${sourceName}: ${title}`)
  })

  it('tells a screen reader the numbers the card words leave out, and the keys', () => {
    const { text } = mount(hand.slice(0, 3), 1, 1, 'Discard a card.')
    expect(text()).toBe(
      `${sourceName}: Discard a card. Choose 1 of 3. Press 1 to 3 to choose a card or take it back.`,
    )
  })

  it('tells the progress as cards are chosen', async () => {
    const { text, card, user } = mount(hand, 2, 3, 'Discard cards.')
    await user.click(card(0))
    expect(text()).toBe(`${sourceName}: 1 of 2 chosen. Choose 1 more.`)
    await user.click(card(1))
    expect(text()).toBe(`${sourceName}: 2 cards chosen. Press Confirm, or choose up to 1 more.`)
    await user.click(card(2))
    expect(text()).toBe(`${sourceName}: 3 cards chosen. Press Confirm, or take one back.`)
  })
})

describe('the cards on offer', () => {
  it('are drawn from the views, in the order offered, with a key for the first ten', () => {
    const { cards } = mount(hand, 1, 1)
    expect(cards().map((c) => c.getAttribute('aria-label'))).toEqual(hand.map((id) => nameOf(id)))
    expect(cards().map((c) => c.getAttribute('aria-keyshortcuts'))).toEqual([
      '1',
      '2',
      '3',
      '4',
      '5',
      '6',
      '7',
    ])
  })

  it('tell two cards of one name apart for a screen reader', () => {
    const { cards } = mount(permanents.slice(0, 12), 1, 1)
    const labels = cards().map((c) => c.getAttribute('aria-label'))
    expect(new Set(labels).size).toBe(labels.length)
  })

  it('are never a second card with the id of the table: the prompt cards carry their own attribute', () => {
    const { container } = mount(hand, 1, 1)
    expect(container.querySelectorAll('[data-decision] [data-object-id]')).toHaveLength(0)
    expect(container.querySelectorAll('[data-decision] [data-candidate]')).toHaveLength(hand.length)
  })

  it('offer a card once however often the engine lists it', () => {
    const [a, b] = hand as [ObjectId, ObjectId]
    expect(mount([a, b, a], 1, 1).cards()).toHaveLength(2)
  })

  it('stand in face down for a card the view does not hold', async () => {
    const { cards, user, confirm, answer } = mount([9_999_001, hand[0] as ObjectId], 1, 1)
    expect(cards()).toHaveLength(2)
    expect(cards()[0]).toHaveTextContent('Card')
    await user.click(must(cards()[0]))
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseCards: [9_999_001] })
  })

  it('have no cards to draw and a Confirm that sends none when nothing is on offer and none is asked', async () => {
    const { confirm, user, answer, count } = mount([], 0, 1)
    expect(count()).toBe('No cards to choose from')
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ ChooseCards: [] })
  })
})

describe('a choice of one', () => {
  it('starts with Confirm closed and focus on the first card, which the clock would take', async () => {
    const { confirm, card } = mount(hand.slice(0, 3), 1, 1, 'Discard a card.')
    expect(confirm()).toBeDisabled()
    expect(card(0)).toHaveFocus()
  })

  it('takes a card, changes to another, takes it back and sends the card once', async () => {
    const { card, confirm, chosen, user, answer, sent } = mount(hand.slice(0, 3), 1, 1)
    await user.click(card(0))
    expect(chosen()).toEqual([hand[0]])
    expect(confirm()).toBeEnabled()
    await user.click(card(2))
    expect(chosen()).toEqual([hand[2]])
    await user.click(card(2))
    expect(chosen()).toEqual([])
    expect(confirm()).toBeDisabled()
    await user.click(card(1))
    await user.dblClick(confirm())
    expect(answer).toHaveBeenCalledTimes(1)
    expect(sent()).toEqual({ ChooseCards: [hand[1]] })
  })

  it('shows what is chosen by a check, a pressed state and data, not by colour alone', async () => {
    const { card, user } = mount(hand.slice(0, 3), 1, 1)
    await user.click(card(1))
    expect(card(1)).toHaveAttribute('aria-pressed', 'true')
    expect(card(1)).toHaveAttribute('data-chosen')
    expect(card(1).querySelector('[data-slot="chosen-mark"]')).not.toBeNull()
    expect(card(0).querySelector('[data-slot="chosen-mark"]')).toBeNull()
  })
})

describe('a choice of several', () => {
  it('opens Confirm at the fewest, closes the rest at the most and sends the order chosen', async () => {
    const { card, confirm, chosen, count, user, answer, sent } = mount(hand, 2, 2)
    await user.click(card(4))
    expect(confirm()).toBeDisabled()
    expect(count()).toBe('Choose 2 of 7 · 1 chosen')
    await user.click(card(1))
    expect(confirm()).toBeEnabled()
    // Chosen cards are listed in the order of the list; what is sent is in the order they were chosen.
    expect(chosen()).toEqual([hand[1], hand[4]])
    // The rest are closed, and a press on one changes nothing.
    expect(card(0)).toHaveAttribute('aria-disabled', 'true')
    await user.click(card(0))
    expect(chosen()).toEqual([hand[1], hand[4]])
    await user.click(confirm())
    expect(answer).toHaveBeenCalledTimes(1)
    expect(sent()).toEqual({ ChooseCards: [hand[4], hand[1]] })
  })

  it('reopens the rest when a card is taken back', async () => {
    const { card, confirm, user } = mount(hand, 2, 2)
    await user.click(card(0))
    await user.click(card(1))
    await user.click(card(0))
    expect(confirm()).toBeDisabled()
    expect(card(2)).not.toHaveAttribute('aria-disabled')
  })

  it('lets up to some be none: Confirm is open at once and sends nothing', async () => {
    const { confirm, user, answer, sent } = mount(hand, 0, 2)
    expect(confirm()).toBeEnabled()
    await user.click(confirm())
    expect(answer).toHaveBeenCalledTimes(1)
    expect(sent()).toEqual({ ChooseCards: [] })
  })

  it('holds a choice of at least one to one card and keeps adding to the most on offer', async () => {
    const { card, cards, confirm, user } = mount(hand.slice(0, 3), 1, 9)
    expect(confirm()).toBeDisabled()
    await user.click(card(0))
    await user.click(card(1))
    await user.click(card(2))
    expect(confirm()).toBeEnabled()
    expect(cards().filter((c) => c.getAttribute('aria-disabled') === 'true')).toHaveLength(0)
  })

  it('never asks for more than is on offer', async () => {
    const { card, confirm, user, answer, sent } = mount(hand.slice(0, 2), 3, 3)
    await user.click(card(0))
    await user.click(card(1))
    expect(confirm()).toBeEnabled()
    await user.click(confirm())
    expect(answer).toHaveBeenCalledTimes(1)
    expect(sent()).toEqual({ ChooseCards: [hand[0], hand[1]] })
  })
})

describe('the number keys', () => {
  it('choose and take back the card at their place, and not a card that is not there', async () => {
    const { user, chosen, answer } = mount(hand.slice(0, 3), 1, 2)
    await user.keyboard('9')
    expect(chosen()).toEqual([])
    await user.keyboard('2')
    expect(chosen()).toEqual([hand[1]])
    await user.keyboard('3')
    expect(chosen()).toEqual([hand[1], hand[2]])
    await user.keyboard('1')
    expect(chosen()).toEqual([hand[1], hand[2]])
    await user.keyboard('2')
    expect(chosen()).toEqual([hand[2]])
    expect(answer).not.toHaveBeenCalled()
  })

  it('reach the first ten of a long list, with 0 for the tenth', async () => {
    const { user, chosen } = mount(permanents.slice(0, 14), 0, 14)
    await user.keyboard('10')
    expect(chosen()).toEqual([permanents[0], permanents[9]])
  })

  it('leave the digits to a text field', async () => {
    const { user, chosen } = mount(permanents.slice(0, 14), 0, 14)
    const field = screen.getByRole('searchbox', { name: 'Filter cards' })
    await user.click(field)
    await user.keyboard('1')
    expect(chosen()).toEqual([])
    expect(field).toHaveValue('1')
  })
})

describe('the keys that move along the cards', () => {
  it('give Tab one card of the list and move it with the arrow keys, Home and End', async () => {
    const { cards, user } = mount(hand, 1, 1)
    const tabbable = (): number => cards().filter((c) => c.tabIndex === 0).length
    expect(tabbable()).toBe(1)
    await user.keyboard('{ArrowRight}')
    expect(cards()[1]).toHaveFocus()
    expect(cards()[1]?.tabIndex).toBe(0)
    expect(tabbable()).toBe(1)
    await user.keyboard('{End}')
    expect(cards()[6]).toHaveFocus()
    await user.keyboard('{Home}')
    expect(cards()[0]).toHaveFocus()
  })

  it('choose the focused card with Enter and with Space', async () => {
    const { user, chosen } = mount(hand, 0, 3)
    await user.keyboard('{Enter}')
    expect(chosen()).toEqual([hand[0]])
    await user.keyboard('{ArrowRight}{ }')
    expect(chosen()).toEqual([hand[0], hand[1]])
  })
})

describe('a long list', () => {
  const long = permanents.slice(0, 20)

  it('gets a filter box from thirteen cards, and a short one gets none', () => {
    mount(permanents.slice(0, 12), 1, 1)
    expect(screen.queryByRole('searchbox', { name: 'Filter cards' })).toBeNull()
  })

  it('narrows to the cards whose name or type matches, and keeps the choice made out of sight', async () => {
    const { cards, user, container } = mount(long, 0, 3)
    expect(screen.getByRole('searchbox', { name: 'Filter cards' })).toBeInTheDocument()
    const all = cards().length
    expect(all).toBe(20)
    await user.click(cards()[0] as HTMLElement)
    const field = screen.getByRole('searchbox', { name: 'Filter cards' })
    await user.type(field, 'creature')
    const shown = cards().map((c) => Number(c.getAttribute('data-candidate')))
    const creatures = long.filter((id) =>
      fixture.view.objects.some((o) => o.id === id && o.types.includes('Creature')),
    )
    expect(shown).toEqual(creatures)
    // The card chosen before still counts, in sight or not.
    expect(must(container.querySelector('[data-slot="cards-count"]')).textContent).toContain('1 chosen')
    // A filter that matches nothing says so.
    await user.clear(field)
    await user.type(field, 'zzzz')
    expect(container.querySelector('[data-slot="no-match"]')).not.toBeNull()
    await user.clear(field)
    expect(cards()).toHaveLength(20)
  })

  it('gives the number keys to the cards in view', async () => {
    const { cards, user, chosen } = mount(long, 0, 3)
    const field = screen.getByRole('searchbox', { name: 'Filter cards' })
    await user.type(field, 'creature')
    const first = Number(must(cards()[0]).getAttribute('data-candidate'))
    ;(document.activeElement as HTMLElement).blur()
    await user.keyboard('1')
    expect(chosen()).toEqual([first])
  })

  it('draws a long list with the compact face and a short one with the whole face', () => {
    expect(mount(long, 0, 3).card(0)).toHaveAttribute('data-size', 'compact')
  })
})

describe('the table', () => {
  it('marks the cards on the battlefield that are on offer as targets are marked, and dims the others of the viewer', async () => {
    const { marks, tableCard, user, confirm, answer, sent } = mount(mine, 1, 1, 'Sacrifice a creature.')
    const own = ownOf(fixture)
    const expected = new Map<ObjectId, string>(own.map((id) => [id, mine.includes(id) ? 'legal' : 'dimmed']))
    expect(marks()).toEqual(expected)
    // A click on the table's card chooses it, as a click on the prompt's does.
    await user.click(tableCard(mine[1] as ObjectId))
    expect(marks().get(mine[1] as ObjectId)).toBe('selected')
    expect(confirm()).toBeEnabled()
    await user.click(confirm())
    expect(sent()).toEqual({ ChooseCards: [mine[1]] })
    expect(answer).toHaveBeenCalledTimes(1)
  })

  it('closes the table cards when the choice is full, and keeps the chosen ones to take back', async () => {
    const { marks, user, cards } = mount(mine, 1, 2)
    await user.click(must(cards()[0]))
    await user.click(must(cards()[1]))
    const states = marks()
    expect(states.get(mine[0] as ObjectId)).toBe('selected')
    expect(states.get(mine[1] as ObjectId)).toBe('selected')
    expect(states.get(mine[2] as ObjectId)).toBe('dimmed')
  })

  it('marks a hand that is on offer, as a discard does', () => {
    const { marks } = mount(hand, 1, 1)
    for (const id of hand) expect(marks().get(id)).toBe('legal')
  })
})
