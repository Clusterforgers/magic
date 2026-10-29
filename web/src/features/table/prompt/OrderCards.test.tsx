/** Goal: for 0 to 6 cards the order prompt starts in the order offered, Up and Down move a card one place and say where it went, the ends of the list have no move past them, focus stays on the button pressed, Alt with the arrows moves the focused card, Original order puts the list back, Confirm order sends every card once in the order shown and sends once, rows name their card and tell equal names apart, and the table's cards are not marked. */
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'

const fixture = fixtureOf(2)
const hand = must(fixture.view.players[0]?.hand.cards)
const source = must(creaturesOf(fixture, 1)[0])
const sourceName = must(fixture.view.objects.find((o) => o.id === source)).name
const nameOf = (id: ObjectId): string => must(fixture.view.objects.find((o) => o.id === id)).name

function mount(cards: ObjectId[], text?: string) {
  const table = renderDecision({ OrderCards: { source, cards, ...(text === undefined ? {} : { text }) } })
  const list = (): HTMLElement => screen.getByRole('list', { name: /^Your cards in order/ })
  /** The card at each place of the list, top first. */
  const shown = (): ObjectId[] =>
    within(list())
      .queryAllByRole('listitem')
      .map((row) => Number(row.getAttribute('data-card')))
  const rowOf = (id: ObjectId): HTMLElement =>
    must(list().querySelector<HTMLElement>(`li[data-card="${id}"]`))
  const move = (id: ObjectId, way: 'up' | 'down'): HTMLElement =>
    must(list().querySelector<HTMLElement>(`button[data-card="${id}"][data-move="${way}"]`))
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm order' })
  const reset = (): HTMLElement => screen.getByRole('button', { name: 'Original order' })
  return { ...table, cards, list, shown, rowOf, move, confirm, reset }
}

describe('the list at first', () => {
  it.each([2, 3, 4, 6])('of %i cards is in the order offered, and Confirm order sends it', async (count) => {
    const cards = hand.slice(0, count)
    const { shown, rowOf, confirm, answer, user, marks } = mount(cards)
    expect(shown()).toEqual(cards)
    for (const id of cards) expect(rowOf(id)).toHaveTextContent(nameOf(id))
    // Nothing on the table is clickable while the cards are put in order.
    expect(marks().size).toBe(0)
    await user.dblClick(confirm())
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer.mock.calls[0]?.[0]).toEqual({ OrderCards: cards })
  })

  it('marks the two ends and has Original order closed', () => {
    const cards = hand.slice(0, 3)
    const { rowOf, reset } = mount(cards)
    expect(rowOf(must(cards[0]))).toHaveTextContent('1.')
    expect(rowOf(must(cards[0]))).toHaveTextContent('(first)')
    expect(rowOf(must(cards[2]))).toHaveTextContent('3.')
    expect(rowOf(must(cards[2]))).toHaveTextContent('(last)')
    expect(rowOf(must(cards[1]))).not.toHaveTextContent('(first)')
    expect(rowOf(must(cards[1]))).not.toHaveTextContent('(last)')
    expect(reset()).toBeDisabled()
  })

  it.each([
    ['Put the cards on the bottom in any order.', `${sourceName}: Put the cards on the bottom in any order.`],
    [undefined, `${sourceName}: Put the 3 cards in order.`],
  ])('with the words %j reads "%s" and tells a screen reader where the list starts', (text, sentence) => {
    const { text: said } = mount(hand.slice(0, 3), text)
    expect(said()).toBe(
      `${sentence} The top card is first. Alt with the arrow keys moves the focused card; a number key goes to the card at that place.`,
    )
  })

  it('names a lone card and sends it', async () => {
    const { text, confirm, user, answer } = mount([must(hand[0])])
    expect(text()).toContain('Put the card in order.')
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ OrderCards: [hand[0]] })
  })

  it('sends an empty order when there is nothing to put in order', async () => {
    const { confirm, user, answer } = mount([])
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ OrderCards: [] })
  })
})

describe('moving a card', () => {
  const [a, b, c, d] = hand.slice(0, 4) as [ObjectId, ObjectId, ObjectId, ObjectId]
  // Each case presses Up or Down on a card; the list starts [a, b, c, d].
  it.each([
    ['a Down', [[a, 'down']], [b, a, c, d]],
    ['b Up', [[b, 'up']], [b, a, c, d]],
    [
      'd Up twice',
      [
        [d, 'up'],
        [d, 'up'],
      ],
      [a, d, b, c],
    ],
    [
      'a Down to the end',
      [
        [a, 'down'],
        [a, 'down'],
        [a, 'down'],
      ],
      [b, c, d, a],
    ],
    [
      'c Up then a Down',
      [
        [c, 'up'],
        [a, 'down'],
      ],
      [c, a, b, d],
    ],
  ] as const)('%s', async (_what, presses, expected) => {
    const { move, shown, confirm, reset, user, answer } = mount([a, b, c, d])
    for (const [id, way] of presses) await user.click(move(id, way))
    expect(shown()).toEqual(expected)
    expect(reset()).toBeEnabled()
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ OrderCards: expected })
  })

  it('says where the card went', async () => {
    const { move, text, user } = mount([a, b, c, d])
    await user.click(move(d, 'up'))
    expect(text()).toContain(`${nameOf(d)} is now 3rd of 4.`)
    await user.click(move(d, 'up'))
    await user.click(move(d, 'up'))
    expect(text()).toContain(`${nameOf(d)} is now 1st of 4, first.`)
  })

  it('keeps focus on the button pressed, also when the row moves', async () => {
    const { move, user } = mount([a, b, c, d])
    await user.click(move(c, 'up'))
    expect(move(c, 'up')).toHaveFocus()
    await user.click(move(a, 'down'))
    expect(move(a, 'down')).toHaveFocus()
  })

  it('has no move past an end: the button reads as unavailable and a press changes nothing', async () => {
    const { move, shown, user } = mount([a, b, c, d])
    expect(move(a, 'up')).toHaveAttribute('aria-disabled', 'true')
    expect(move(d, 'down')).toHaveAttribute('aria-disabled', 'true')
    await user.click(move(a, 'up'))
    await user.click(move(d, 'down'))
    expect(shown()).toEqual([a, b, c, d])
  })

  it('puts the list back with Original order', async () => {
    const { move, shown, reset, text, user } = mount([a, b, c, d])
    await user.click(move(d, 'up'))
    await user.click(move(a, 'down'))
    await user.click(reset())
    expect(shown()).toEqual([a, b, c, d])
    expect(reset()).toBeDisabled()
    expect(text()).not.toContain('is now')
  })

  it('moves the focused card with Alt and the arrow keys, and not with the arrow keys alone', async () => {
    const { move, shown, user } = mount([a, b, c, d])
    move(c, 'down').focus()
    await user.keyboard('{ArrowUp}')
    expect(shown()).toEqual([a, b, c, d])
    await user.keyboard('{Alt>}{ArrowUp}{/Alt}')
    expect(shown()).toEqual([a, c, b, d])
    expect(move(c, 'up')).toHaveFocus()
    await user.keyboard('{Alt>}{ArrowDown}{/Alt}')
    expect(shown()).toEqual([a, b, c, d])
  })
})

describe('equal names', () => {
  it('are numbered, so a button names its own card', () => {
    const lands = fixture.view.objects.filter(
      (o) => o.zone === 'Battlefield' && o.controller === 0 && o.types.includes('Land'),
    )
    // The first land that has an earlier one of the same name, and that earlier one.
    const second = must(lands.find((land, place) => lands.findIndex((o) => o.name === land.name) < place))
    const first = must(lands.find((o) => o.name === second.name))
    const { move } = mount([first.id, second.id])
    expect(move(first.id, 'down')).toHaveAccessibleName(`Move ${first.name} 1 down`)
    expect(move(second.id, 'up')).toHaveAccessibleName(`Move ${first.name} 2 up`)
  })
})
