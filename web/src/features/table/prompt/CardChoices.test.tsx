/** Goal: every prompt that picks cards draws them as pictures in the one grid (a graveyard, exile, the stack and the battlefield among them, never only names) at a size by how many are on offer, and the card chosen stays large at the side in the table's preview (by touch too), the card under the pointer replaces it only while hovered, and Escape or an empty choice takes it away. */
import { fireEvent, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, ObjectView, Target } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { NORMAL_UP_TO, tileSizeFor } from './PromptCard'
import { restingCard } from './restingPreview'

const fixture = fixtureOf(2)
const find = (test: (object: ObjectView) => boolean): ObjectView => must(fixture.view.objects.find(test))
const inGraveyard = (object: ObjectView): boolean =>
  typeof object.zone === 'object' && 'Graveyard' in object.zone && object.owner === 1
const spell = find((object) => object.zone === 'Stack' && object.controller === 1)
const buried = find(inGraveyard)
const exiled = find((object) => object.zone === 'Exile')
const hand = must(fixture.view.players[0]?.hand.cards)
const source = find((object) => object.zone === 'Battlefield' && object.controller === 1).id
const nameOf = (id: ObjectId): string => must(fixture.view.objects.find((object) => object.id === id)).name

const preview = (): HTMLElement | null => screen.queryByTestId('card-preview')
const shown = (): string => within(must(preview())).getAllByText(/./)[0]?.textContent ?? ''

describe('the size of a tile', () => {
  it.each([
    [1, 'normal'],
    [NORMAL_UP_TO, 'normal'],
    [NORMAL_UP_TO + 1, 'compact'],
    [40, 'compact'],
  ])('%i cards on offer are drawn %s', (count, size) => {
    expect(tileSizeFor(count)).toBe(size)
  })

  it.each([
    [[], null],
    [[4], 4],
    [[4, 9, 2], 2],
  ])('the card resting in the preview of the choice %j is %j', (chosen, rest) => {
    expect(restingCard(chosen)).toBe(rest)
  })
})

describe('targets the table does not draw', () => {
  const legal: Target[] = [spell.id, buried.id, exiled.id].map((id) => ({ Object: { id, counter: 0 } }))
  const mount = () =>
    renderDecision({
      ChooseTargets: {
        source: spell.id,
        spec: 0,
        legal,
        min: 1,
        max: 1,
        can_cancel: true,
        wording: 'target card',
      },
    })

  it('are cards with their picture, in a graveyard, in exile and on the stack, not only names', () => {
    mount()
    const group = screen.getByRole('group', { name: 'Legal targets' })
    const cards = within(group).getAllByRole('button')
    expect(cards.map((card) => card.getAttribute('data-candidate'))).toEqual(
      [spell.id, buried.id, exiled.id].map(String),
    )
    for (const card of cards) expect(card.querySelector('[data-slot="card-face"]')).not.toBeNull()
  })

  it('show the chosen card large at the side, also when it was chosen by touch', () => {
    mount()
    const card = must(
      screen
        .getByRole('group', { name: 'Legal targets' })
        .querySelector<HTMLElement>(`[data-candidate="${buried.id}"]`),
    )
    fireEvent.click(card)
    expect(shown()).toContain(buried.name)
    expect(card).toHaveAttribute('aria-pressed', 'true')
  })

  it('take the preview away when the choice is taken back', () => {
    mount()
    const card = must(
      screen
        .getByRole('group', { name: 'Legal targets' })
        .querySelector<HTMLElement>(`[data-candidate="${buried.id}"]`),
    )
    fireEvent.click(card)
    expect(preview()).not.toBeNull()
    fireEvent.click(card)
    expect(preview()).toBeNull()
  })
})

describe('cards chosen from a list', () => {
  const mount = () =>
    renderDecision({
      ChooseCards: { source, from: hand.slice(0, 4), min: 1, max: 2, text: 'Discard cards.' },
    })
  const card = (id: ObjectId): HTMLElement =>
    must(
      screen
        .getByRole('group', { name: 'Cards to choose from' })
        .querySelector<HTMLElement>(`[data-candidate="${id}"]`),
    )

  it('rest large at the side: the last one chosen, whichever was chosen first', () => {
    mount()
    fireEvent.click(card(must(hand[0])))
    expect(shown()).toContain(nameOf(must(hand[0])))
    fireEvent.click(card(must(hand[1])))
    expect(shown()).toContain(nameOf(must(hand[1])))
  })

  it('give the preview to the card under the pointer and back to the chosen one when it leaves', () => {
    mount()
    const [first, second] = [must(hand[0]), must(hand[1])]
    fireEvent.click(card(first))
    fireEvent.pointerEnter(card(second), { pointerType: 'mouse' })
    expect(shown()).toContain(nameOf(second))
    fireEvent.pointerLeave(card(second), { pointerType: 'mouse' })
    fireEvent.pointerOut(card(second), { pointerType: 'mouse' })
    expect(shown()).toContain(nameOf(first))
  })

  it('go when Escape closes the preview, and when the prompt is answered', () => {
    const { answer } = mount()
    fireEvent.click(card(must(hand[0])))
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(preview()).toBeNull()
    fireEvent.click(card(must(hand[0])))
    expect(preview()).toBeNull()
    expect(answer).not.toHaveBeenCalled()
  })
})

describe('a discard from the hand', () => {
  it('rests the last card chosen large at the side while the hand stays in the dock', () => {
    const { card } = renderDecision({ Discard: { hand: hand.slice(0, 3), count: 2 } })
    fireEvent.click(card(must(hand[1])))
    expect(shown()).toContain(nameOf(must(hand[1])))
  })
})
