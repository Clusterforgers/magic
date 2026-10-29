/** Goal: `?prompt=replacement|replacement-two|scry|scry-one|scry-order|scry-bottom|put-back` and the clocks give the viewer, at every seat count, a decision the wire parser accepts whose cards (the library cards of a scry, the hand cards put back, the sources of the effects) are in the view, and each is drawn on the dev table and answered, which clears it. */
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { parseMessage } from '@/net/parse'
import { must } from '@/test-utils/batches'
import { fixtureGame } from './devGame'

/** The prompt, the decision it asks and the button that answers it. */
const PROMPTS = [
  ['replacement', 'ChooseReplacement', 3],
  ['replacement-two', 'ChooseReplacement', 2],
  ['replacement-clock', 'ChooseReplacement', 3],
  ['scry', 'ChooseCards', 4],
  ['scry-one', 'ChooseCards', 1],
  ['scry-clock', 'ChooseCards', 4],
  ['scry-order', 'OrderCards', 4],
  ['scry-bottom', 'OrderCards', 3],
  ['put-back', 'OrderCards', 2],
] as const

describe.each(PROMPTS)('?prompt=%s', (prompt, name, count) => {
  it.each([2, 4, 12])('faces %s with its cards in the view at %i seats', (seats) => {
    const { pending, view } = fixtureGame({ seats, prompt }).session.store.getState()
    const { decision, id } = must(pending)
    const kind = decision.kind
    expect(decision.player).toBe(0)
    expect(name in kind).toBe(true)
    const ids = new Set(must(view).objects.map((o) => o.id))
    const cards =
      'ChooseReplacement' in kind
        ? kind.ChooseReplacement.candidates.map((c) => c.source)
        : 'ChooseCards' in kind
          ? kind.ChooseCards.from
          : 'OrderCards' in kind
            ? kind.OrderCards.cards
            : []
    expect(cards).toHaveLength(count)
    for (const card of cards) expect(ids.has(card)).toBe(true)
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
    expect(document.querySelector(`[data-decision="${name}"]`)).not.toBeNull()
    if (name === 'ChooseReplacement') await user.keyboard('1')
    else
      await user.click(
        screen.getByRole('button', { name: name === 'OrderCards' ? 'Confirm order' : 'Confirm' }),
      )
    expect(document.querySelector('[data-decision]')).toBeNull()
    expect(game.session.store.getState().pending).toBeNull()
  })
})

describe('the clocks', () => {
  it('run on the clock variants only', () => {
    const clock = (prompt: string) => fixtureGame({ seats: 2, prompt }).session.store.getState().clock
    expect(clock('replacement-clock')).not.toBeNull()
    expect(clock('scry-clock')).not.toBeNull()
    expect(clock('scry')).toBeNull()
  })
})
