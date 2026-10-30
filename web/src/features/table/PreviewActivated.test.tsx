// Goal: the preview of a card lists its activated abilities in the card's words, without its mana abilities, and tags the ones the Priority prompt offers as available now; it lists them for an opponent's card too, but marks none there, and says "No abilities." only for a card with nothing to list.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { indexById } from '@/play/view'
import type { PlayState } from '@/play/store'
import type { AbilityRef, ActivatedView, ObjectId, ObjectView, PriorityOptions } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'

const fixture = buildFixture({ seats: 2 })
const objects = fixture.view.objects
const mine = must(
  objects.find(
    (object) =>
      object.zone === 'Battlefield' &&
      object.controller === 0 &&
      object.types.includes('Creature') &&
      object.abilities.length === 0,
  ),
)
const theirs = must(
  objects.find(
    (object) => object.zone === 'Battlefield' && object.controller === 1 && object.types.includes('Creature'),
  ),
)
const land = must(
  objects.find(
    (object) => object.zone === 'Battlefield' && object.controller === 0 && object.abilities.length > 0,
  ),
)

const at = (object: ObjectView, ability: number): AbilityRef => ({ object: object.id, ability })
const draw = (object: ObjectView): ActivatedView => ({
  ability: at(object, 0),
  cost: '{2}, {T}',
  effect: 'Draw a card.',
})
const equip = (object: ObjectView): ActivatedView => ({
  ability: at(object, 1),
  cost: '{1}',
  effect: 'Attach to target creature you control.',
  keyword: 'Equip {1}',
})
const tap: ActivatedView = { cost: '{T}', effect: 'Add {G}.', mana: true }

/** The table of seat 0 with `words` on the cards, facing priority that offers `offered`, or no decision. */
function renderPreview(words: [ObjectView, ...ActivatedView[]][], offered: AbilityRef[] | null) {
  const options: PriorityOptions = {
    playable_lands: [],
    castable: [],
    mana_abilities: [],
    abilities: offered ?? [],
    can_act: true,
    can_pass: true,
  }
  const table = renderTable({
    fixture,
    state: (state: PlayState) => {
      const view = clone(must(state.view))
      for (const [object, ...activated] of words)
        must(view.objects.find((o) => o.id === object.id)).activated = activated
      return {
        ...state,
        view,
        objectsById: indexById(view.objects),
        pending: offered === null ? null : { id: 4, decision: { player: 0, kind: { Priority: options } } },
      }
    },
  })
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  const preview = () => screen.getByTestId('card-preview')
  const list = (): HTMLElement | null => preview().querySelector('[data-slot="preview-activated"]')
  const lines = (): string[] =>
    [...(list()?.querySelectorAll('li') ?? [])].map((item) => (item.firstChild?.textContent ?? '').trim())
  const now = (): string[] =>
    [...(list()?.querySelectorAll('li[data-now]') ?? [])].map((item) =>
      (item.firstChild?.textContent ?? '').trim(),
    )
  return { ...table, card, preview, list, lines, now }
}

describe('the activated abilities in the preview', () => {
  it('lists them in the card’s words, in the order of the card', async () => {
    const { card, user, lines, list } = renderPreview([[mine, draw(mine), equip(mine)]], [])
    await user.hover(card(mine.id))
    expect(list()).toHaveAccessibleName('Activated abilities')
    expect(lines()).toEqual([
      '{2}, {T}: Draw a card.',
      'Equip {1} ({1}: Attach to target creature you control.)',
    ])
  })

  it('tags the ones the prompt offers now, and no others', async () => {
    const { card, user, now, preview } = renderPreview([[mine, draw(mine), equip(mine)]], [at(mine, 1)])
    await user.hover(card(mine.id))
    expect(now()).toEqual(['Equip {1} ({1}: Attach to target creature you control.)'])
    expect(preview().querySelectorAll('[data-slot="preview-now"]')).toHaveLength(1)
    expect(screen.getByText('Can activate now')).toBeInTheDocument()
  })

  it('tags none while the viewer holds no decision', async () => {
    const { card, user, now, lines } = renderPreview([[mine, draw(mine)]], null)
    await user.hover(card(mine.id))
    expect(lines()).toHaveLength(1)
    expect(now()).toEqual([])
  })

  it('leaves out a mana ability, which the mana line tells', async () => {
    const { card, user, lines, preview } = renderPreview([[land, tap, draw(land)]], [])
    await user.hover(card(land.id))
    expect(lines()).toEqual(['{2}, {T}: Draw a card.'])
    expect(preview()).toHaveTextContent('{T}: Add')
  })

  it('has no list for a card that only taps for mana', async () => {
    const { card, user, list } = renderPreview([[land, tap]], [])
    await user.hover(card(land.id))
    expect(list()).toBeNull()
  })

  it('lists an opponent’s card too, with nothing available', async () => {
    const { card, user, lines, now } = renderPreview([[theirs, draw(theirs)]], [at(theirs, 0)])
    await user.hover(card(theirs.id))
    expect(lines()).toEqual(['{2}, {T}: Draw a card.'])
    expect(now()).toEqual([])
  })

  it('says "No abilities." only for a card with nothing to list', async () => {
    const plain = renderPreview([[mine, draw(mine)]], [])
    await plain.user.hover(plain.card(mine.id))
    expect(plain.preview()).not.toHaveTextContent('No abilities.')
    plain.unmount()
    const bare = renderPreview([[mine]], [])
    await bare.user.hover(bare.card(mine.id))
    expect(bare.preview()).toHaveTextContent('No abilities.')
    expect(bare.list()).toBeNull()
  })

  it('is read as one sentence by a screen reader: the words, then that it can be activated now', async () => {
    const { card, user, list } = renderPreview([[mine, draw(mine)]], [at(mine, 0)])
    await user.hover(card(mine.id))
    expect(list()).toHaveTextContent('{2}, {T}: Draw a card.Can activate now')
  })
})
