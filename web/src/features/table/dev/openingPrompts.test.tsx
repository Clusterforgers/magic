/** Goal: `?prompt=mulligan|mulligan-free|bottom|discard` hands the viewer, at every seat count, a decision the wire parser accepts and whose cards are the viewer's real hand (discard leaves one card out so a dimmed card shows), and each can be rendered and answered on the dev table. */
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { parseMessage } from '@/net/parse'
import { must } from '@/test-utils/batches'
import { fixtureGame } from './devGame'

const NAMES = ['mulligan', 'mulligan-free', 'bottom', 'discard'] as const
const KIND = {
  mulligan: 'Mulligan',
  'mulligan-free': 'Mulligan',
  bottom: 'BottomCards',
  discard: 'Discard',
} as const

describe.each(NAMES)('?prompt=%s', (prompt) => {
  it.each([2, 4, 12])('faces a decision of the viewer’s real hand at %i seats', (seats) => {
    const { pending, view } = fixtureGame({ seats, prompt }).session.store.getState()
    const { decision, id } = must(pending)
    expect(decision.player).toBe(0)
    expect(KIND[prompt] in decision.kind).toBe(true)
    const hand = must(must(view).players[0]?.hand.cards)
    const options = Object.values(decision.kind)[0] as { hand: number[] }
    for (const card of options.hand) expect(hand).toContain(card)
    expect(parseMessage({ type: 'Decision', id, decision }).ok).toBe(true)
  })

  it('is drawn on the table and answered, which clears it', async () => {
    const game = fixtureGame({ seats: 2, prompt })
    render(
      <GameContext value={game}>
        <TableOptionsContext value={{}}>
          <GameTable />
        </TableOptionsContext>
      </GameContext>,
    )
    const user = userEvent.setup()
    expect(document.querySelector(`[data-decision="${KIND[prompt]}"]`)).not.toBeNull()
    if (KIND[prompt] === 'Mulligan') await user.click(screen.getByRole('button', { name: 'Keep' }))
    else {
      const [first, second] = [...document.querySelectorAll<HTMLElement>('[data-highlight="legal"]')]
      await user.click(must(first))
      await user.click(must(second))
      await user.click(screen.getByRole('button', { name: 'Confirm' }))
    }
    expect(document.querySelector('[data-decision]')).toBeNull()
    expect(game.session.store.getState().pending).toBeNull()
  })
})

describe('?prompt=discard', () => {
  it('leaves the last card of the hand out, so a dimmed hand card can be seen', () => {
    const { pending, view } = fixtureGame({ seats: 2, prompt: 'discard' }).session.store.getState()
    const hand = must(must(view).players[0]?.hand.cards)
    const kind = must(pending).decision.kind
    const offered = 'Discard' in kind ? kind.Discard.hand : []
    expect(offered).not.toContain(hand.at(-1))
    expect(offered).toHaveLength(hand.length - 1)
  })
})
