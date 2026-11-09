// Goal: the hand dock's sort menu and the table menu's "Hand order" put the viewer's hand in the chosen order and remember it, Alt with an arrow key, Home or End moves the focused card, keeps its focus and says where it went, and a custom order takes new cards at the end, drops cards that left and lasts through a reload of the game.
import { act, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { CardType, ObjectView } from '@/protocol/generated'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import { clone, must } from '@/test-utils/batches'
import { card } from '@/test-utils/cards'
import { renderTable } from '@/test-utils/tableHarness'
import { HAND_SORT_KEY, handOrderKey, resetHandOrder } from './useHandOrder'

const HAND: ReadonlyArray<[number, string, CardType, number]> = [
  [9001, 'Shock', 'Instant', 1],
  [9002, 'Mountain', 'Land', 0],
  [9003, 'Centaur', 'Creature', 3],
  [9004, 'Bloom', 'Enchantment', 4],
  [9005, 'Forest', 'Land', 0],
]

const handCard = ([id, name, type, value]: (typeof HAND)[number]): ObjectView =>
  card({
    id,
    owner: 0,
    controller: 0,
    zone: { Hand: 0 },
    permanent: null,
    name,
    types: [type],
    mana_cost: value === 0 ? null : [{ Generic: value }],
  })

/** The table with `cards` in the viewer's hand, in that server order. */
function withHand(cards: ReadonlyArray<(typeof HAND)[number]>): (state: PlayState) => PlayState {
  return (state) => {
    const view = clone(must(state.view))
    view.objects = [
      ...view.objects.filter((object) => !(typeof object.zone === 'object' && 'Hand' in object.zone)),
      ...cards.map(handCard),
    ]
    must(view.players[0]).hand = { count: cards.length, cards: cards.map(([id]) => id) }
    must(view.players[0]).commanders = undefined
    return { ...state, view, objectsById: indexById(view.objects) }
  }
}

const names = (): string[] =>
  [
    ...screen
      .getByRole('list', { name: "Alice's hand" })
      .querySelectorAll<HTMLElement>('button[data-object-id]'),
  ].map((button) => must(HAND.find(([id]) => `${id}` === button.dataset.objectId))[1])

const sortButton = (): HTMLElement => screen.getByRole('button', { name: /^Sort hand:/ })

beforeEach(() => {
  localStorage.clear()
  sessionStorage.clear()
  resetHandOrder()
})
afterEach(() => {
  localStorage.clear()
  sessionStorage.clear()
})

describe('table menu', () => {
  it('sorts the hand from "Hand order" and shows the order in force, Custom checking none', async () => {
    const table = renderTable({ seats: 2, state: withHand(HAND) })
    await table.user.click(screen.getByRole('button', { name: 'Table menu' }))
    const group = await screen.findByRole('group', { name: 'Hand order' })
    expect(within(group).getByRole('button', { name: 'As drawn' })).toHaveAttribute('aria-pressed', 'true')
    await table.user.click(within(group).getByRole('button', { name: 'Mana value' }))
    expect(names()).toEqual(['Forest', 'Mountain', 'Shock', 'Centaur', 'Bloom'])
    expect(sortButton()).toHaveAccessibleName('Sort hand: Mana value')
    act(() => must(document.querySelector<HTMLElement>('button[data-object-id="9004"]')).focus())
    await table.user.keyboard('{Alt>}{Home}{/Alt}')
    expect(sortButton()).toHaveAccessibleName('Sort hand: Custom')
    if (screen.queryByRole('group', { name: 'Hand order' }) === null)
      await table.user.click(screen.getByRole('button', { name: 'Table menu' }))
    const again = await screen.findByRole('group', { name: 'Hand order' })
    for (const button of within(again).getAllByRole('button')) {
      expect(button).toHaveAttribute('aria-pressed', 'false')
    }
  })
})

describe('sort menu', () => {
  it.each([
    ['Lands, creatures, spells', 'types', ['Forest', 'Mountain', 'Centaur', 'Shock', 'Bloom']],
    ['Mana value', 'mana', ['Forest', 'Mountain', 'Shock', 'Centaur', 'Bloom']],
    ['As drawn', 'drawn', ['Shock', 'Mountain', 'Centaur', 'Bloom', 'Forest']],
  ])('%s orders the hand and is remembered', async (label, key, want) => {
    const table = renderTable({ seats: 2, state: withHand(HAND) })
    expect(names()).toEqual(['Shock', 'Mountain', 'Centaur', 'Bloom', 'Forest'])
    expect(sortButton()).toHaveAccessibleName('Sort hand: As drawn')
    await table.user.click(sortButton())
    await table.user.click(await screen.findByRole('menuitemradio', { name: label }))
    expect(names()).toEqual(want)
    expect(sortButton()).toHaveAccessibleName(`Sort hand: ${label}`)
    expect(localStorage.getItem(HAND_SORT_KEY)).toBe(key)
    table.unmount()
    resetHandOrder()
    renderTable({ seats: 2, state: withHand(HAND) })
    expect(names()).toEqual(want)
  })

  it('puts a drawn card into place under an auto sort', () => {
    localStorage.setItem(HAND_SORT_KEY, 'types')
    const table = renderTable({ seats: 2, state: withHand(HAND.slice(0, 4)) })
    expect(names()).toEqual(['Mountain', 'Centaur', 'Shock', 'Bloom'])
    table.update(withHand(HAND))
    expect(names()).toEqual(['Forest', 'Mountain', 'Centaur', 'Shock', 'Bloom'])
  })

  it('reads a stored value that is no sort as "As drawn"', () => {
    localStorage.setItem(HAND_SORT_KEY, 'bogus')
    renderTable({ seats: 2, state: withHand(HAND) })
    expect(names()).toEqual(['Shock', 'Mountain', 'Centaur', 'Bloom', 'Forest'])
  })
})

describe('keyboard reorder', () => {
  const focusCard = (name: string): HTMLElement => {
    const entry = must(HAND.find((each) => each[1] === name))
    const button = must(document.querySelector<HTMLElement>(`button[data-object-id="${entry[0]}"]`))
    act(() => button.focus())
    return button
  }

  it.each([
    ['Shock', '{Alt>}{ArrowRight}{/Alt}', ['Mountain', 'Shock', 'Centaur', 'Bloom', 'Forest'], 2],
    ['Centaur', '{Alt>}{ArrowLeft}{/Alt}', ['Shock', 'Centaur', 'Mountain', 'Bloom', 'Forest'], 2],
    ['Shock', '{Alt>}{End}{/Alt}', ['Mountain', 'Centaur', 'Bloom', 'Forest', 'Shock'], 5],
    ['Forest', '{Alt>}{Home}{/Alt}', ['Forest', 'Shock', 'Mountain', 'Centaur', 'Bloom'], 1],
  ])('%s with %s', async (name, keys, want, place) => {
    const table = renderTable({ seats: 2, state: withHand(HAND) })
    const moved = focusCard(name)
    await table.user.keyboard(keys)
    expect(names()).toEqual(want)
    expect(document.activeElement).toBe(moved)
    expect(sortButton()).toHaveAccessibleName('Sort hand: Custom')
    expect(
      within(screen.getByRole('region', { name: 'Your hand' })).getByText(
        `${name} moved to place ${place} of 5.`,
      ),
    ).toBeInTheDocument()
  })

  it('leaves the order alone at the end of the hand and moves focus without Alt', async () => {
    const table = renderTable({ seats: 2, state: withHand(HAND) })
    focusCard('Forest')
    await table.user.keyboard('{Alt>}{ArrowRight}{/Alt}')
    expect(sortButton()).toHaveAccessibleName('Sort hand: As drawn')
    await table.user.keyboard('{ArrowLeft}')
    expect(names()).toEqual(['Shock', 'Mountain', 'Centaur', 'Bloom', 'Forest'])
    expect(document.activeElement?.getAttribute('data-object-id')).toBe('9004')
  })

  it('keeps a custom order per game: drawn cards at the end, gone cards out, through a reload', async () => {
    const table = renderTable({ seats: 2, state: withHand(HAND.slice(0, 4)) })
    focusCard('Bloom')
    await table.user.keyboard('{Alt>}{Home}{/Alt}')
    expect(names()).toEqual(['Bloom', 'Shock', 'Mountain', 'Centaur'])
    table.update(withHand(HAND.filter(([, name]) => name !== 'Mountain')))
    expect(names()).toEqual(['Bloom', 'Shock', 'Centaur', 'Forest'])
    expect(sessionStorage.getItem(handOrderKey('test-game'))).not.toBeNull()
    table.unmount()
    resetHandOrder()
    const again = renderTable({ seats: 2, state: withHand(HAND.filter(([, name]) => name !== 'Mountain')) })
    expect(names()).toEqual(['Bloom', 'Shock', 'Centaur', 'Forest'])
    await again.user.click(sortButton())
    await again.user.click(await screen.findByRole('menuitemradio', { name: 'As drawn' }))
    expect(names()).toEqual(['Shock', 'Centaur', 'Bloom', 'Forest'])
    expect(sessionStorage.getItem(handOrderKey('test-game'))).toBeNull()
  })
})
