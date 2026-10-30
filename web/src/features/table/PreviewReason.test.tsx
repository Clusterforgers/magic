// Goal: the preview of a dimmed hand card says why it cannot be played, in the line of the server's reason or of the table's own guess, on hover, on focus and when a tap pins it (touch has no hover), and the preview of a playable card, a battlefield card or any card outside a Priority prompt says nothing of the kind.
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, PriorityOptions, Unplayable } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { REASON_WORDS } from './handReason'

const fixture = buildFixture({ seats: 2 })
const hand = must(fixture.view.players[0]?.hand.cards)
const spell = must(hand.find((id) => !fixture.view.objects.find((o) => o.id === id)?.types.includes('Land')))
const land = must(hand.find((id) => fixture.view.objects.find((o) => o.id === id)?.types.includes('Land')))
const onBoard = must(
  fixture.view.objects.find((object) => object.zone === 'Battlefield' && object.controller === 0),
).id

const options = (over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_act: false,
  can_pass: true,
  ...over,
})

function renderPriority(given: PriorityOptions | null) {
  const table = renderTable({
    state: (state) =>
      given === null
        ? { ...state, pending: null }
        : { ...state, pending: { id: 4, decision: { player: 0, kind: { Priority: given } } } },
  })
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  const reason = (): string | null =>
    screen.queryByTestId('card-preview')?.querySelector('[data-slot="preview-reason"]')?.textContent ?? null
  return { ...table, card, reason }
}

describe('the line of the reason in the preview of a dimmed hand card', () => {
  const told: Unplayable[] = ['Timing', 'LandPlayed', 'Mana', 'Targets', 'Cost', 'Restricted']

  it.each(told)('says what the server told: %s', async (why) => {
    const { card, user, reason } = renderPriority(options({ unplayable: [{ card: spell, reason: why }] }))
    await user.hover(card(spell))
    expect(reason()).toBe(`${REASON_WORDS[why].line}.`)
  })

  it('says what the table can tell when the server named no reason', async () => {
    const { card, user, reason } = renderPriority(options())
    await user.hover(card(spell))
    expect(reason()).toMatch(/^[A-Z].*\.$/)
    expect(Object.values(REASON_WORDS).map((words) => `${words.line}.`)).toContain(reason())
  })

  it('is shown on keyboard focus', async () => {
    const { card, reason } = renderPriority(options({ unplayable: [{ card: spell, reason: 'Mana' }] }))
    act(() => card(spell).focus())
    expect(reason()).toBe('Not enough mana.')
  })

  it('is reached by a tap, which pins the preview of a card that cannot be played', async () => {
    const { card, user, reason } = renderPriority(options({ unplayable: [{ card: spell, reason: 'Mana' }] }))
    await user.click(card(spell))
    expect(screen.getByTestId('card-preview')).toHaveAttribute('data-pinned', 'true')
    expect(reason()).toBe('Not enough mana.')
  })

  it('sits under the name, before the rest of the card’s text', async () => {
    const { card, user } = renderPriority(options({ unplayable: [{ card: spell, reason: 'Mana' }] }))
    await user.hover(card(spell))
    const preview = screen.getByTestId('card-preview')
    const line = must(preview.querySelector('[data-slot="preview-reason"]'))
    expect(line.previousElementSibling).toHaveClass('font-semibold')
  })
})

describe('a preview with nothing to say about why', () => {
  const NOTHING: [string, PriorityOptions | null, () => ObjectId][] = [
    ['a card that can be cast', options({ castable: [spell] }), () => spell],
    ['a land that can be played', options({ playable_lands: [land] }), () => land],
    ['a permanent that is dimmed', options(), () => onBoard],
    ['a hand card outside a Priority prompt', null, () => spell],
  ]
  it.each(NOTHING)('%s', async (_name, given, id) => {
    const { card, user, reason } = renderPriority(given)
    await user.hover(card(id()))
    expect(screen.getByTestId('card-preview')).toBeInTheDocument()
    expect(reason()).toBeNull()
  })

  it('loses the line when the prompt goes', async () => {
    const { card, user, reason, update } = renderPriority(
      options({ unplayable: [{ card: spell, reason: 'Mana' }] }),
    )
    await user.hover(card(spell))
    expect(reason()).toBe('Not enough mana.')
    update(() => ({ pending: null }))
    expect(reason()).toBeNull()
  })
})
