/** Goal: the order of cards going back on a library is drawn as pictures from the top down with the end of the library named from the engine's words, the line under it previews the result, Alt with the arrow keys (up or left, down or right) and the buttons move a card, a number key goes to the card at that place, and Confirm sends every card once for 0, 1, 2, 3 and 5 cards, whether they come from a scry, a surveil or a hand. */
import { screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf } from '@/test-utils/combatTable'
import { renderTable } from '@/test-utils/tableHarness'
import { LOOKED_MOMENTS } from '../dev/lookedPrompts'

const TOP = 'Put the cards on top of your library in any order.'
const BOTTOM = 'Put the cards on the bottom of your library in any order.'

const looked = must(LOOKED_MOMENTS['scry'])(fixtureOf(2))
const library = looked.view.objects
  .filter((o) => typeof o.zone === 'object' && 'Library' in o.zone)
  .map((o) => o.id)
const nameOf = (id: ObjectId): string => must(looked.view.objects.find((o) => o.id === id)).name
const source = must(creaturesOf(fixtureOf(2), 0)[0])

function mount(cards: ObjectId[], text = TOP) {
  const answer = vi.fn((_answer: Answer) => true)
  const table = renderTable({
    fixture: looked,
    session: { answer },
    state: (state) => ({
      ...state,
      pending: { id: 7, decision: { player: 0, kind: { OrderCards: { source, cards, text } } } },
    }),
  })
  const root = table.container
  const shown = (): ObjectId[] =>
    [...root.querySelectorAll('[data-slot="card-order"] > li')].map((li) =>
      Number(li.getAttribute('data-card')),
    )
  const move = (id: ObjectId, way: 'up' | 'down'): HTMLElement =>
    must(root.querySelector<HTMLElement>(`button[data-card="${id}"][data-move="${way}"]`))
  const preview = (): string | null => root.querySelector('[data-slot="order-preview"]')?.textContent ?? null
  const pile = (): string => must(root.querySelector('[data-slot="order-pile"]')).textContent ?? ''
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm order' })
  return { ...table, answer, root, shown, move, preview, pile, confirm }
}

describe('the strip', () => {
  it.each([
    [TOP, 'top', 'Going on top of your library: first on the left'],
    [BOTTOM, 'bottom', 'Going on the bottom of your library: first on the left'],
  ])('for the words "%s" names the %s end of the library', (words, end, heading) => {
    const { pile, root } = mount(library.slice(0, 3), words)
    expect(pile()).toBe(heading)
    expect(root.querySelector('[data-slot="card-order"]')).toHaveAttribute('data-pile', end)
  })

  it('draws every card as a picture and the table has no picture for a card it does not hold', () => {
    const cards = library.slice(0, 3)
    const { root } = mount(cards)
    expect(root.querySelectorAll('[data-slot="card-order"] [data-slot="prompt-picture"]')).toHaveLength(3)
  })

  it.each([
    [TOP, 'Top of your library: '],
    [BOTTOM, 'Bottom of your library, top to bottom: '],
  ])('previews the result for "%s" from the top down and follows a move', async (words, head) => {
    const [a, b, c] = library as [ObjectId, ObjectId, ObjectId]
    const { preview, move, user } = mount([a, b, c], words)
    expect(preview()).toBe(`${head}${[a, b, c].map(nameOf).join(', then ')}.`)
    await user.click(move(c, 'up'))
    await user.click(move(c, 'up'))
    expect(preview()).toBe(`${head}${[c, a, b].map(nameOf).join(', then ')}.`)
  })

  it('has no preview for nothing to order', () => {
    expect(mount([]).preview()).toBeNull()
  })
})

describe('Confirm order', () => {
  // The cards are reversed with the buttons, last card up to the top each time, then sent.
  it.each([0, 1, 2, 3, 5])('sends %i cards once, each once, in the order shown', async (count) => {
    const cards = library.slice(0, count)
    const { move, shown, confirm, user, answer } = mount(cards)
    for (let place = 1; place < count; place += 1) {
      const id = must(cards[place])
      for (let step = 0; step < place; step += 1) await user.click(move(id, 'up'))
    }
    const reversed = [...cards].reverse()
    expect(shown()).toEqual(reversed)
    await user.dblClick(confirm())
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer.mock.calls[0]?.[0]).toEqual({ OrderCards: reversed })
  })

  it('sends cards put back from the hand in the order chosen', async () => {
    const hand = must(fixtureOf(2).view.players[0]?.hand.cards).slice(0, 2) as [ObjectId, ObjectId]
    const { move, confirm, user, answer } = mount(hand)
    await user.click(move(hand[1], 'up'))
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ OrderCards: [hand[1], hand[0]] })
  })
})

describe('the keys', () => {
  const [a, b, c, d] = library as [ObjectId, ObjectId, ObjectId, ObjectId]

  it.each([
    ['Alt and ArrowLeft', '{Alt>}{ArrowLeft}{/Alt}', [a, c, b, d]],
    ['Alt and ArrowUp', '{Alt>}{ArrowUp}{/Alt}', [a, c, b, d]],
    ['Alt and ArrowRight', '{Alt>}{ArrowRight}{/Alt}', [a, b, d, c]],
    ['Alt and ArrowDown', '{Alt>}{ArrowDown}{/Alt}', [a, b, d, c]],
    ['ArrowLeft alone', '{ArrowLeft}', [a, b, c, d]],
  ])('%s moves the focused card c', async (_what, keys, expected) => {
    const { move, shown, user } = mount([a, b, c, d])
    move(c, 'down').focus()
    await user.keyboard(keys)
    expect(shown()).toEqual(expected)
  })

  it.each([
    ['1', a, 'down'],
    ['3', c, 'up'],
    ['4', d, 'up'],
  ] as const)('the number key %s goes to the card at that place', async (key, id, button) => {
    const { move, user } = mount([a, b, c, d])
    await user.keyboard(key)
    expect(move(id, button)).toHaveFocus()
  })

  it('ignores a number past the last card and leaves the digits to a text field', async () => {
    const { move, user, shown } = mount([a, b])
    move(b, 'down').focus()
    await user.keyboard('5')
    expect(move(b, 'down')).toHaveFocus()
    const field = document.createElement('input')
    document.body.append(field)
    field.focus()
    await user.keyboard('1')
    expect(field).toHaveFocus()
    field.remove()
    expect(shown()).toEqual([a, b])
  })
})
