/** Goal: `?prompt=divide|divide-three|costs|costs-gift|unknown-kind` hand the viewer, at every seat count, a decision the wire parser accepts whose source, targets and gift seats are on the table, and each can be drawn and answered on the dev table (the division and the costs with their own prompts, the unknown kind with the generic one), which clears the decision. */
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { parseMessage } from '@/net/parse'
import { must } from '@/test-utils/batches'
import { SEAT_BASE } from '../prompt/costsText'
import { fixtureGame } from './devGame'

const KNOWN = ['divide', 'divide-three', 'costs', 'costs-gift'] as const

function table(seats: number, prompt: string) {
  const game = fixtureGame({ seats, prompt })
  render(
    <GameContext value={game}>
      <TableOptionsContext value={{}}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  return game
}

describe.each([...KNOWN, 'unknown-kind'])('?prompt=%s', (prompt) => {
  it.each([2, 4, 12])('hands the viewer a decision of real cards and players at %i seats', (seats) => {
    const { pending, view } = fixtureGame({ seats, prompt }).session.store.getState()
    const { decision, id } = must(pending)
    expect(decision.player).toBe(0)
    expect(parseMessage({ type: 'Decision', id, decision }).ok).toBe(true)
    const ids = new Set(must(view).objects.map((o) => o.id))
    const kind = decision.kind
    if ('DivideAmount' in kind) {
      const { source, total, among } = kind.DivideAmount
      expect(ids.has(source)).toBe(true)
      expect(among.length).toBeGreaterThan(0)
      expect(total).toBeGreaterThanOrEqual(among.length)
      for (const target of among) {
        if ('Player' in target) expect(target.Player).toBeLessThan(seats)
        else expect(ids.has(target.Object.id)).toBe(true)
      }
    } else if ('ChooseCosts' in kind) {
      const { source, optional } = kind.ChooseCosts
      expect(ids.has(source)).toBe(true)
      expect(optional.length).toBeGreaterThan(0)
      for (const cost of optional.filter((o) => o.index >= SEAT_BASE)) {
        expect(cost.index - SEAT_BASE).toBeLessThan(seats)
        expect(cost.index - SEAT_BASE).not.toBe(0)
      }
    } else {
      expect(Object.keys(kind)).toEqual(['Frobnicate'])
    }
  })
})

describe('drawn and answered on the dev table', () => {
  it.each([
    ['divide', 'DivideAmount', 'Confirm'],
    ['divide-three', 'DivideAmount', 'Confirm'],
    ['costs', 'ChooseCosts', 'Confirm costs'],
  ])('%s: the prompt of %s and %s clears it', async (prompt, name, button) => {
    const game = table(2, prompt)
    const user = userEvent.setup()
    expect(document.querySelector(`[data-decision="${name}"]`)).not.toBeNull()
    await user.click(screen.getByRole('button', { name: button }))
    expect(document.querySelector('[data-decision]')).toBeNull()
    expect(game.session.store.getState().pending).toBeNull()
  })

  it('costs-gift: a gift to each opponent, one at most, and the answer holds it', async () => {
    const game = table(4, 'costs-gift')
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Gift a card to player 2' }))
    await user.click(screen.getByRole('button', { name: 'Gift a card to player 3' }))
    expect(document.querySelector('[data-slot="costs-problem"]')).not.toBeNull()
    await user.click(screen.getByRole('button', { name: 'Confirm costs' }))
    expect(game.session.store.getState().pending).toBeNull()
  })

  it('unknown-kind: the generic prompt, with the key as its name and no answer to give', () => {
    table(2, 'unknown-kind')
    expect(document.querySelector('[data-decision="Frobnicate"]')).not.toBeNull()
    expect(screen.getByText('This decision is not supported in the UI yet')).toBeInTheDocument()
  })
})
