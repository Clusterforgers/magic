/** Goal: the Activate row of a mana ability that does more than add mana (`PayOptions.announced`, `?prompt=pay-announced`) shows in every home of the payment, the full dock, the short dock and a phone's pay sheet, beside Confirm, and sends one Activate answer for its ability. */
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import type { Answer } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { fixtureGame } from '../dev/devGame'
import { GameTable } from '../GameTable'
import { TableOptionsContext } from '../tableOptions'

const HOMES = [
  ['the full dock', { width: 1600, height: 1000 }, 'full'],
  ['the short dock', { width: 1100, height: 800 }, 'short'],
  ['a phone', { width: 390, height: 844 }, 'compact'],
] as const

/** `?prompt=pay-announced` on a table of this size, every answer going to a spy. */
function table(viewport: { width: number; height: number }) {
  const game = fixtureGame({ seats: 2, board: 'duel', prompt: 'pay-announced' })
  const pending = must(game.session.store.getState().pending)
  const kind = pending.decision.kind
  if (!('PayMana' in kind)) throw new Error('pay-announced is not a payment')
  const answer = vi.fn((_answer: Answer) => true)
  render(
    <GameContext value={{ ...game, session: { ...game.session, answer } }}>
      <TableOptionsContext value={{ viewport }}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  return { answer, announced: must(kind.PayMana.announced), user: userEvent.setup() }
}

describe.each(HOMES)('the Activate row in %s', (_name, viewport, form) => {
  it('stands with the payment and sends one Activate answer for its ability', async () => {
    const { answer, announced, user } = table(viewport)
    const dock = document.querySelector('[data-dock-form]')
    if (form !== 'compact') expect(dock).toHaveAttribute('data-dock-form', form)
    const activate = must(document.querySelector<HTMLButtonElement>('[data-slot="pay-activate"]'))
    expect(activate).toBeVisible()
    expect(activate).toHaveAccessibleName(/^Activate /)
    expect(screen.getByRole('button', { name: 'Confirm payment' })).toBeInTheDocument()
    await user.click(activate)
    expect(answer).toHaveBeenCalledTimes(1)
    expect(answer).toHaveBeenCalledWith({ PayMana: { Activate: must(announced[0]) } })
  })
})
