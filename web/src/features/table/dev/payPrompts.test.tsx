/** Goal: `?prompt=pay|pay-short|pay-locked|pay-hybrid|pay-low-life|pay-sources` hands the viewer, at every seat count, a payment decision the wire parser accepts, and the two hybrid ones show a cost with every kind of symbol, the life row with a switch for each Phyrexian symbol, and a payment that can be completed with life and sent. */
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { parseMessage } from '@/net/parse'
import type { PayOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { costText } from '../cardModel'
import { fixtureGame } from './devGame'

const NAMES = ['pay', 'pay-short', 'pay-locked', 'pay-hybrid', 'pay-low-life', 'pay-sources'] as const

function options(prompt: string, seats = 2): PayOptions {
  const { pending } = fixtureGame({ seats, prompt }).session.store.getState()
  const kind = must(pending).decision.kind
  if (!('PayMana' in kind)) throw new Error(`${prompt} is not a payment`)
  return kind.PayMana
}

describe.each(NAMES)('?prompt=%s', (prompt) => {
  it.each([2, 4, 12])('faces a payment the wire parser accepts at %i seats', (seats) => {
    const { pending } = fixtureGame({ seats, prompt }).session.store.getState()
    const { decision, id } = must(pending)
    expect(decision.player).toBe(0)
    expect('PayMana' in decision.kind).toBe(true)
    expect(parseMessage({ type: 'Decision', id, decision }).ok).toBe(true)
  })

  it('says the life total and the most life that may be paid', () => {
    const given = options(prompt)
    const symbols = given.cost.filter(
      (symbol) => typeof symbol === 'object' && ('Phyrexian' in symbol || 'HybridPhyrexian' in symbol),
    )
    expect(given.max_life).toBeLessThanOrEqual(2 * symbols.length)
    expect(given.max_life).toBeLessThanOrEqual(given.life)
  })
})

describe('?prompt=pay-hybrid', () => {
  it('is a cost with a hybrid, a monocolored hybrid, a Phyrexian and a hybrid Phyrexian symbol', () => {
    expect(costText(options('pay-hybrid').cost)).toBe('{1}{G/W}{2/U}{W/P}{G/U/P}')
  })

  it('gives the spell in the hand that cost too, so its card draws the symbols', () => {
    const game = fixtureGame({ seats: 2, prompt: 'pay-hybrid' })
    const view = must(game.session.store.getState().view)
    const hand = must(view.players[0]?.hand.cards)
    const spell = must(
      view.objects.find((object) => object.name === 'Selesnya Evangel' && hand.includes(object.id)),
    )
    expect(costText(must(spell.mana_cost))).toBe('{1}{G/W}{2/U}{W/P}{G/U/P}')
    expect(options('pay-hybrid').spell).toBe(spell.id)
  })

  it('has 20 life and room for 4', () => {
    const given = options('pay-hybrid')
    expect([given.life, given.max_life]).toEqual([20, 4])
  })

  it('is drawn on the table, paid with life and answered', async () => {
    const game = fixtureGame({ seats: 2, prompt: 'pay-hybrid' })
    const answer = vi.spyOn(game.session, 'answer')
    render(
      <GameContext value={game}>
        <TableOptionsContext value={{}}>
          <GameTable />
        </TableOptionsContext>
      </GameContext>,
    )
    const user = userEvent.setup()
    expect(document.querySelector('[data-decision="PayMana"]')).not.toBeNull()
    const row = screen.getByRole('group', { name: 'Life' })
    expect(within(row).getByText('You have 20 life.')).toBeInTheDocument()
    const switches = within(row).getAllByRole('button')
    expect(switches.map((button) => button.getAttribute('aria-label'))).toEqual([
      'Pay 2 life instead of white mana',
      'Pay 2 life instead of green or blue mana',
    ])
    await user.click(must(switches[0]))
    await user.click(must(switches[1]))
    expect(within(row).getByText('You have 20 life, 16 after paying 4.')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Pay the rest for me' }))
    expect(answer.mock.calls).toEqual([[{ PayMana: { Complete: { pool: [], taps: [], life: 4 } } }]])
    expect(document.querySelector('[data-decision]')).toBeNull()
  })
})

describe('?prompt=pay-low-life', () => {
  it('is the same cost with 3 life, which pays for one symbol and not both', async () => {
    const given = options('pay-low-life')
    expect([given.life, given.max_life]).toEqual([3, 2])
    const game = fixtureGame({ seats: 2, prompt: 'pay-low-life' })
    render(
      <GameContext value={game}>
        <TableOptionsContext value={{}}>
          <GameTable />
        </TableOptionsContext>
      </GameContext>,
    )
    const user = userEvent.setup()
    const [first, second] = within(screen.getByRole('group', { name: 'Life' })).getAllByRole('button')
    await user.click(must(first))
    expect(must(second)).toHaveAttribute('aria-disabled', 'true')
  })
})

describe('the plain payment prompts', () => {
  it.each(['pay', 'pay-short', 'pay-locked'])('%s has no life to pay and says so', (prompt) => {
    const given = options(prompt)
    expect(given.max_life).toBe(0)
    expect(
      given.cost.every((symbol) => typeof symbol === 'string' || 'Generic' in symbol || 'Colored' in symbol),
    ).toBe(true)
  })
})
