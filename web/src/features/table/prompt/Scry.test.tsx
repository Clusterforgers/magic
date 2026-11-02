/** Goal: for 1 to 5 cards the scry prompt starts with every card on top in the order looked at, a card is sent to the other pile and back by its button or its number key, moves within its pile by its buttons or Alt with the arrows, is dragged between the piles and into place, the line under the piles previews the result, Original order puts it back, and Confirm sends both piles, each in the order shown, once: all on top, all on the bottom and mixed. */
import { fireEvent, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'

const fixture = fixtureOf(2)
const hand = must(fixture.view.players[0]?.hand.cards)
const source = must(creaturesOf(fixture, 1)[0])
const nameOf = (id: ObjectId): string => must(fixture.view.objects.find((o) => o.id === id)).name

function mount(count: number) {
  const cards = hand.slice(0, count)
  const table = renderDecision({ Scry: { source, cards } })
  const pile = (name: 'top' | 'bottom'): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`[data-slot="scry-${name}"]`))
  const shown = (name: 'top' | 'bottom'): ObjectId[] =>
    within(pile(name))
      .queryAllByRole('listitem')
      .map((row) => Number(row.getAttribute('data-card')))
  const button = (id: ObjectId, way: 'up' | 'down' | 'cross'): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-card="${id}"][data-move="${way}"]`))
  const row = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`li[data-card="${id}"]`))
  const preview = (): string =>
    must(table.container.querySelector('[data-slot="scry-preview"]')).textContent ?? ''
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm' })
  const reset = (): HTMLElement => screen.getByRole('button', { name: 'Original order' })
  const drag = (id: ObjectId, onto: HTMLElement): void => {
    const data = new Map<string, string>()
    const dataTransfer = {
      setData: (type: string, value: string) => data.set(type, value),
      getData: (type: string) => data.get(type) ?? '',
      effectAllowed: 'all',
    }
    fireEvent.dragStart(row(id), { dataTransfer })
    fireEvent.drop(onto, { dataTransfer })
  }
  return { ...table, cards, pile, shown, button, row, preview, confirm, reset, drag }
}

describe.each([1, 2, 3, 4, 5])('a scry of %i cards', (count) => {
  it('starts with every card on top and Confirm sends them once in that order', async () => {
    const { cards, shown, confirm, reset, answer, user } = mount(count)
    expect(shown('top')).toEqual(cards)
    expect(shown('bottom')).toEqual([])
    expect(reset()).toBeDisabled()
    await user.dblClick(confirm())
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer.mock.calls[0]?.[0]).toEqual({ Scry: { top: cards, bottom: [] } })
  })

  it('sends every card to the bottom, in the order they were sent', async () => {
    const { cards, shown, button, confirm, answer, user } = mount(count)
    for (const id of cards) await user.click(button(id, 'cross'))
    expect(shown('top')).toEqual([])
    expect(shown('bottom')).toEqual(cards)
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ Scry: { top: [], bottom: cards } })
  })

  it('keeps the order of both piles when the cards are split and reordered', async () => {
    const { cards, shown, button, confirm, reset, answer, user } = mount(count)
    const [first, ...rest] = cards
    // The first card goes down; the rest stay and the last one moves to the front of its pile.
    await user.click(button(must(first), 'cross'))
    const last = rest.at(-1)
    if (last !== undefined && rest.length > 1) await user.click(button(last, 'up'))
    const top = rest.length > 1 ? [...rest.slice(0, -2), must(last), must(rest.at(-2))] : rest
    expect(shown('top')).toEqual(top)
    expect(shown('bottom')).toEqual([must(first)])
    expect(reset()).toBeEnabled()
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ Scry: { top, bottom: [must(first)] } })
  })
})

describe('the controls', () => {
  it('sends a card across and back by its button, which keeps focus, and the sentence says where it went', async () => {
    const { cards, button, user, container, shown } = mount(3)
    const [a, b] = cards as [ObjectId, ObjectId, ObjectId]
    await user.click(button(b, 'cross'))
    expect(button(b, 'cross')).toHaveFocus()
    expect(container).toHaveTextContent(`${nameOf(b)} is on the bottom, now 1st of 1`)
    await user.click(button(a, 'cross'))
    expect(shown('bottom')).toEqual([b, a])
    await user.click(button(b, 'cross'))
    expect(shown('top')).toEqual([cards[2], b])
  })

  it('has no move past either end of a pile', () => {
    const { cards, button } = mount(3)
    expect(button(must(cards[0]), 'up')).toHaveAttribute('aria-disabled', 'true')
    expect(button(must(cards[2]), 'down')).toHaveAttribute('aria-disabled', 'true')
    expect(button(must(cards[1]), 'up')).not.toHaveAttribute('aria-disabled', 'true')
  })

  it('moves the focused card with Alt and the arrows', async () => {
    const { cards, button, shown, user } = mount(3)
    button(must(cards[0]), 'down').focus()
    await user.keyboard('{Alt>}{ArrowRight}{/Alt}')
    expect(shown('top')).toEqual([cards[1], cards[0], cards[2]])
    await user.keyboard('{Alt>}{ArrowLeft}{/Alt}')
    expect(shown('top')).toEqual(cards)
  })

  it('sends the card looked at in that place with its number key', async () => {
    const { cards, shown, user } = mount(4)
    await user.keyboard('2')
    await user.keyboard('4')
    expect(shown('top')).toEqual([cards[0], cards[2]])
    expect(shown('bottom')).toEqual([cards[1], cards[3]])
    await user.keyboard('2')
    expect(shown('bottom')).toEqual([cards[3]])
  })

  it('previews the result from the top down and Original order puts it back', async () => {
    const { cards, button, preview, reset, shown, confirm, user, answer } = mount(3)
    const [a, b, c] = cards as [ObjectId, ObjectId, ObjectId]
    expect(preview()).toBe(
      `Top of your library: ${nameOf(a)}, then ${nameOf(b)}, then ${nameOf(c)}. Bottom, top to bottom: none.`,
    )
    await user.click(button(a, 'cross'))
    expect(preview()).toContain(`Bottom, top to bottom: ${nameOf(a)}.`)
    await user.click(reset())
    expect(shown('top')).toEqual(cards)
    expect(shown('bottom')).toEqual([])
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({ Scry: { top: cards, bottom: [] } })
  })
})

describe('dragging', () => {
  it('drops a card on the other pile, last there', async () => {
    const { cards, pile, drag, shown, confirm, answer, user } = mount(3)
    drag(must(cards[0]), pile('bottom'))
    expect(shown('top')).toEqual([cards[1], cards[2]])
    expect(shown('bottom')).toEqual([cards[0]])
    await user.click(confirm())
    expect(answer.mock.calls[0]?.[0]).toEqual({
      Scry: { top: [cards[1], cards[2]], bottom: [cards[0]] },
    })
  })

  it('drops a card before the card it lands on, in either pile', () => {
    const { cards, pile, row, drag, shown } = mount(4)
    const [a, b, c, d] = cards as [ObjectId, ObjectId, ObjectId, ObjectId]
    drag(d, row(a))
    expect(shown('top')).toEqual([d, a, b, c])
    drag(c, pile('bottom'))
    drag(a, pile('bottom'))
    drag(b, row(c))
    expect(shown('top')).toEqual([d])
    expect(shown('bottom')).toEqual([b, c, a])
  })

  it('drops a card back on top at the front', () => {
    const { cards, pile, row, drag, shown } = mount(3)
    drag(must(cards[0]), pile('bottom'))
    drag(must(cards[0]), row(must(cards[1])))
    expect(shown('top')).toEqual(cards)
    expect(shown('bottom')).toEqual([])
  })
})
