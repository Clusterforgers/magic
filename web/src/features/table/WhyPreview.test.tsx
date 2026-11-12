// Goal: with the server's detail the preview of a dimmed hand card and of a permanent whose abilities cannot be used says the plain sentence of `whyWords`, a target on the stack adds the Full Control hint, and the chooser of a card lists each ability it cannot use now as a disabled line with its reason.
import { screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ObjectId, PriorityOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'

const fixture = buildFixture({ seats: 2 })
const nameOf = (id: ObjectId): string => must(fixture.view.objects.find((object) => object.id === id)).name
const hand = must(fixture.view.players[0]?.hand.cards)
const spell = must(hand.find((id) => !fixture.view.objects.find((o) => o.id === id)?.types.includes('Land')))
const onBoard = must(
  fixture.view.objects.find(
    (object) => object.zone === 'Battlefield' && object.controller === 0 && !object.types.includes('Land'),
  ),
).id

const options = (over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_act: false,
  can_pass: true,
  ...over,
})

function renderPriority(given: PriorityOptions) {
  const answer = vi.fn(() => true)
  const table = renderTable({
    session: { answer },
    state: (state) => ({ ...state, pending: { id: 4, decision: { player: 0, kind: { Priority: given } } } }),
  })
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  const reason = (): string | null =>
    screen.queryByTestId('card-preview')?.querySelector('[data-slot="preview-reason"]')?.textContent ?? null
  return { ...table, card, reason, answer }
}

describe('the sentence of the reason', () => {
  it('names the target a hand card needs, where it is, and how to get one', async () => {
    const { card, user, reason } = renderPriority(
      options({
        unplayable: [
          {
            card: spell,
            reason: 'Targets',
            why: { Target: { needs: 'a creature spell you control on the stack', zone: 'Stack' } },
          },
        ],
      }),
    )
    await user.hover(card(spell))
    expect(reason()).toBe(
      `${nameOf(spell)} needs a target: a creature spell you control on the stack. Cast a creature first and keep priority (Full Control) to target it.`,
    )
  })

  it('names the mana a hand card costs against the mana at hand', async () => {
    const { card, user, reason } = renderPriority(
      options({
        unplayable: [
          {
            card: spell,
            reason: 'Mana',
            why: { Mana: { cost: [{ Generic: 2 }, { Colored: 'Blue' }], at_hand: 1 } },
          },
        ],
      }),
    )
    await user.hover(card(spell))
    expect(reason()).toBe(`${nameOf(spell)} costs {2}{U}; you have 1 mana.`)
  })

  it('says why a permanent cannot use its ability', async () => {
    const { card, user, reason } = renderPriority(
      options({
        inactive: [{ ability: { object: onBoard, ability: 0 }, reason: 'Cost', why: 'SummoningSick' }],
      }),
    )
    await user.hover(card(onBoard))
    expect(reason()).toBe(
      `${nameOf(onBoard)} came under your control this turn, so its {T} abilities wait until your next turn.`,
    )
  })
})

describe('the chooser', () => {
  it('lists an ability that cannot be used now as a disabled line with its reason', async () => {
    const { card, user, answer } = renderPriority(
      options({
        can_act: true,
        abilities: [
          { object: onBoard, ability: 0 },
          { object: onBoard, ability: 1 },
        ],
        inactive: [{ ability: { object: onBoard, ability: 2 }, reason: 'Restricted', why: 'LimitSpent' }],
      }),
    )
    await user.click(card(onBoard))
    const menu = must(document.querySelector<HTMLElement>('[data-slot="ability-chooser"]'))
    const blocked = must(menu.querySelector<HTMLElement>('[data-blocked]'))
    expect(blocked.getAttribute('aria-disabled')).toBe('true')
    expect(within(blocked).getByText('Already used as often as it allows this turn.')).toBeInTheDocument()
    await user.click(blocked)
    expect(answer).not.toHaveBeenCalled()
  })
})
