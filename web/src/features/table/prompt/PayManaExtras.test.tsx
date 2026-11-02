/** Goal: the PayMana prompt spends restricted mana of the pool by hand only on what its restriction allows, shows it with the words of its use, and sends it in the `restricted` field of one Manual answer; it taps creatures for convoke (as {1} or a colour they have) and artifacts for improvise through buttons that leave the offers once chosen, stop while no generic mana is owed and send the choices in the `convoke` and `improvise` fields; it exiles graveyard cards for delve the same way, for generic mana only, and sends them in `delve`. */
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

const mine = fixture.view.objects.filter((object) => object.zone === 'Battlefield' && object.controller === 0)
const nameOf = (id: number): string => must(fixture.view.objects.find((object) => object.id === id)).name
const [first, second, third] = mine.map((object) => object.id)

describe('convoke and improvise in the payment prompt', () => {
  it('taps creatures for {1} or a colour and sends them as convoke', async () => {
    const a = must(first)
    const b = must(second)
    const { answer, table } = renderPay({
      cost: cost('{1}{G}'),
      pool: poolOf(''),
      convoke: [
        { creature: a, colors: ['Green'] },
        { creature: b, colors: [] },
      ],
    })
    await table.user.click(screen.getByRole('button', { name: `Tap ${nameOf(a)} to convoke for green` }))
    expect(screen.getByRole('button', { name: 'Confirm payment' })).toBeDisabled()
    await table.user.click(screen.getByRole('button', { name: `Tap ${nameOf(b)} to convoke for {1}` }))
    await table.user.click(screen.getByRole('button', { name: 'Confirm payment' }))
    expect(answer).toHaveBeenCalledWith({
      PayMana: {
        Manual: {
          pool: [],
          taps: [],
          life: 0,
          convoke: [
            { creature: a, pays: 'Green' },
            { creature: b, pays: null },
          ],
        },
      },
    })
  })

  it('takes a creature out of the offers once it is chosen and gives it back on removal', async () => {
    const a = must(first)
    const { table } = renderPay({
      cost: cost('{1}'),
      pool: poolOf(''),
      convoke: [{ creature: a, colors: ['Green'] }],
    })
    await table.user.click(screen.getByRole('button', { name: `Tap ${nameOf(a)} to convoke for {1}` }))
    expect(screen.queryByRole('button', { name: /to convoke for/ })).toBeNull()
    await table.user.click(screen.getByRole('button', { name: /^Remove 1 generic by tapping/ }))
    expect(screen.getAllByRole('button', { name: /to convoke for/ })).toHaveLength(2)
  })

  it('taps artifacts for improvise, only while generic mana is owed', async () => {
    const [a, b, c] = [must(first), must(second), must(third)]
    const { answer, table } = renderPay({ cost: cost('{2}'), pool: poolOf(''), improvise: [a, b, c] })
    await table.user.click(screen.getByRole('button', { name: `Tap ${nameOf(a)} to improvise for {1}` }))
    await table.user.click(screen.getByRole('button', { name: `Tap ${nameOf(b)} to improvise for {1}` }))
    expect(screen.getByRole('button', { name: `Tap ${nameOf(c)} to improvise for {1}` })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
    await table.user.click(screen.getByRole('button', { name: 'Confirm payment' }))
    expect(answer).toHaveBeenCalledWith({
      PayMana: { Manual: { pool: [], taps: [], life: 0, improvise: [a, b] } },
    })
  })
})

const yard = fixture.view.objects.filter((object) =>
  (must(fixture.view.players[0]).graveyard.cards ?? []).includes(object.id),
)
const unique = yard.filter((one) => yard.filter((other) => other.name === one.name).length === 1)

describe('delve in the payment prompt', () => {
  it('exiles graveyard cards for generic mana and sends them as delve', async () => {
    const [a, b, c] = unique.map((object) => object.id)
    const { answer, table } = renderPay({
      cost: cost('{2}{G}'),
      pool: { ...poolOf('G') },
      delve: [must(a), must(b), must(c)],
    })
    await table.user.click(screen.getByRole('button', { name: `Exile ${nameOf(must(a))} to delve for {1}` }))
    await table.user.click(screen.getByRole('button', { name: /^Spend green from your pool/ }))
    expect(screen.getByRole('button', { name: 'Confirm payment' })).toBeDisabled()
    await table.user.click(screen.getByRole('button', { name: `Exile ${nameOf(must(b))} to delve for {1}` }))
    expect(screen.getByRole('button', { name: `Exile ${nameOf(must(c))} to delve for {1}` })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
    await table.user.click(screen.getByRole('button', { name: 'Confirm payment' }))
    expect(answer).toHaveBeenCalledWith({
      PayMana: { Manual: { pool: ['Green'], taps: [], life: 0, delve: [a, b] } },
    })
  })

  it('does not offer a card for a colour: with only {B} owed nothing is left to exile', () => {
    const [a] = unique.map((object) => object.id)
    renderPay({ cost: cost('{B}'), pool: poolOf(''), delve: [must(a)] })
    expect(screen.getByRole('button', { name: /to delve for/ })).toHaveAttribute('aria-disabled', 'true')
  })
})
