// Goal: where the dock or a phone's bar is the home, a list stands in a window over the table that repeats the question in its foot while the answers stay in the home, one set sent once, and View table (V) shows the table through the window and the window again.
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import type { Answer } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { fixtureGame } from '../dev/devGame'
import { GameTable } from '../GameTable'
import { TableOptionsContext } from '../tableOptions'

/** The dev prompt `prompt` on a table of this size, every answer going to a spy. */
function table(prompt: string, viewport: { width: number; height: number }) {
  const game = fixtureGame({ seats: 2, board: 'duel', prompt })
  const answer = vi.fn((_answer: Answer) => true)
  const view = render(
    <GameContext value={{ ...game, session: { ...game.session, answer } }}>
      <TableOptionsContext value={{ viewport }}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  return { ...view, answer, user: userEvent.setup() }
}

const RAIL = { width: 1100, height: 800 }
const PHONE = { width: 390, height: 844 }

describe('the window of a list', () => {
  it.each([
    ['a rail', RAIL, 'dock'],
    ['a phone', PHONE, 'bar'],
  ] as const)(
    'stands over the table on %s with the question, the answers staying in its home',
    (_name, viewport, home) => {
      table('costs', viewport)
      const window = must(document.querySelector<HTMLElement>('[data-slot="decision-window"]'))
      expect(window).toHaveAttribute('data-home', home)
      expect(window.closest('[data-slot="choice-host"]')).not.toBeNull()
      expect(within(window).getByText(/./, { selector: '[data-slot="window-question"]' })).toBeInTheDocument()
      const line = must(document.querySelector<HTMLElement>('[data-slot="decision-line"]'))
      expect(line).toHaveAttribute('data-home', home)
      // One set of answers: the home keeps them, the window none.
      expect(window.querySelector('[data-slot="prompt-actions"]')).toBeNull()
      expect(line.querySelector('[data-slot="prompt-actions"] button')).not.toBeNull()
    },
  )

  it('draws one Confirm, and sends it once however often it is pressed', async () => {
    const { answer, user } = table('costs', RAIL)
    await user.dblClick(screen.getByRole('button', { name: /^Confirm costs/ }))
    expect(answer).toHaveBeenCalledTimes(1)
  })

  it('shows the table through the window with V and the window again', async () => {
    const { user } = table('mode-two', RAIL)
    const window = (): HTMLElement =>
      must(document.querySelector<HTMLElement>('[data-slot="decision-window"]'))
    expect(window()).not.toHaveAttribute('data-peek')
    await user.keyboard('v')
    expect(window()).toHaveAttribute('data-peek')
    expect(screen.getByRole('button', { name: /Show the window/ })).toHaveAttribute('aria-pressed', 'true')
    await user.keyboard('v')
    expect(window()).not.toHaveAttribute('data-peek')
  })

  it('stays out of a panel that is the home: the list stands under the asking card', () => {
    table('mode-two', { width: 1400, height: 900 })
    expect(document.querySelector('[data-slot="decision-window"]')).toBeNull()
    expect(document.querySelector('[data-home="panel"] [data-slot="mode-list"]')).not.toBeNull()
  })
})
