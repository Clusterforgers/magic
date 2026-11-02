/** Goal: the PayMana prompt spends restricted mana of the pool by hand only on what its restriction allows, shows it with the words of its use, and sends it in the `restricted` field of one Manual answer. */
import { screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, PayOptions, Spend as PaySpend, SpendOn } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { cost, poolOf } from '@/test-utils/mana'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const spell = must(
  fixture.view.objects.find((object) => object.name === 'Selesnya Evangel' && object.owner === 0),
)
const CREATURE = 1 << 2
const creatureSpell: PaySpend = { kind: 'Spell', traits: { types: CREATURE, colors: 1 } }
const instant: PaySpend = { kind: 'Spell', traits: { types: 1 << 4, colors: 1 } }
const creatures: SpendOn[] = [{ Spell: { types: ['Creature'] } }]

function renderPay(over: Partial<PayOptions>) {
  const answer = vi.fn((_answer: Answer) => true)
  const given: PayOptions = {
    spell: spell.id,
    cost: cost('{G}'),
    pool: { ...poolOf(''), restricted: [{ mana: 'Green', only: creatures }] },
    sources: [],
    auto_available: true,
    can_cancel: true,
    life: 20,
    max_life: 0,
    ...over,
  }
  const table = renderTable({
    session: { answer },
    state: (state) => ({ ...state, pending: { id: 4, decision: { player: 0, kind: { PayMana: given } } } }),
  })
  return { answer, table }
}

describe('restricted mana in the payment prompt', () => {
  it('spends it on a spell its restriction allows and sends it', async () => {
    const { answer, table } = renderPay({ spend: creatureSpell })
    const button = screen.getByRole('button', { name: /^Spend restricted green from your pool/ })
    expect(button).toHaveTextContent('only for creature spells')
    expect(button).toHaveAttribute('aria-disabled', 'false')
    await table.user.click(button)
    await table.user.click(screen.getByRole('button', { name: 'Confirm payment' }))
    expect(answer).toHaveBeenCalledWith({
      PayMana: {
        Manual: {
          pool: [],
          restricted: [{ mana: 'Green', only: creatures }],
          taps: [],
          life: 0,
        },
      },
    })
  })

  it.each<[string, PaySpend | undefined]>([
    ['another kind of spell', instant],
    ['a payment for neither', undefined],
  ])('keeps it off %s', async (_name, spend) => {
    const { answer, table } = renderPay(spend === undefined ? {} : { spend })
    const button = screen.getByRole('button', { name: /^Spend restricted green from your pool/ })
    expect(button).toHaveAttribute('aria-disabled', 'true')
    await table.user.click(button)
    expect(screen.getByRole('button', { name: 'Confirm payment' })).toBeDisabled()
    expect(answer).not.toHaveBeenCalled()
  })
})
