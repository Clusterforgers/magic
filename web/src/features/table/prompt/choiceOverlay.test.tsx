// Goal: a choice of cards takes the centre overlay where `wantsOverlay` says so (every choice on a phone): its cards leave the shared strip for an overlay over both boards, the strip moves under it and keeps the sentence and the buttons, and an answer closes it; a wide window keeps the cards in the strip.
import { act, render, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import type { Viewport } from '../layout'
import { CHOICE_AREAS, TABLE_AREAS } from '../layout'
import { GameTable } from '../GameTable'
import { TableOptionsContext } from '../tableOptions'
import { fixtureGame } from '../dev/devGame'
import { must } from '@/test-utils/batches'
import { wantsOverlay } from './choiceOverlay'

const PHONE: Viewport = { width: 390, height: 844 }
const WIDE: Viewport = { width: 1400, height: 900 }

function table(prompt: string, viewport: Viewport) {
  const game = fixtureGame({ seats: 4, prompt })
  const view = render(
    <GameContext value={game}>
      <TableOptionsContext value={{ viewport }}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  const main = must(view.container.querySelector<HTMLElement>('[data-slot="table-main"]'))
  const host = must(view.container.querySelector<HTMLElement>('[data-slot="choice-host"]'))
  const strip = must(view.container.querySelector<HTMLElement>('[data-region="shared"]'))
  return { game, main, host, strip, ...view }
}

describe('wantsOverlay', () => {
  it.each([
    [0, true, false],
    [1, true, true],
    [7, true, true],
    [1, false, false],
    [7, false, false],
  ])('a choice of %i cards on a phone (%s) takes the overlay: %s', (count, phone, wants) => {
    expect(wantsOverlay(count, phone)).toBe(wants)
  })
})

describe.each(['cards', 'cards-battlefield'])('?prompt=%s on a phone', (prompt) => {
  it('draws the cards in the overlay and keeps the sentence and the buttons in the strip under it', () => {
    const { host, strip, main } = table(prompt, PHONE)
    expect(host.hidden).toBe(false)
    const overlay = must(host.querySelector<HTMLElement>('[data-slot="choice-overlay"]'))
    expect(overlay.dataset['decision']).toBe('ChooseCards')
    expect(overlay.querySelectorAll('button[data-candidate]').length).toBeGreaterThan(0)
    expect(strip.querySelector('[data-slot="card-choices"]')).toBeNull()
    expect(strip.querySelector('[data-slot="prompt-text"]')).not.toBeNull()
    expect(within(strip).getByRole('button', { name: /Confirm/ })).toBeInTheDocument()
    expect(main.style.gridTemplateAreas).toBe(CHOICE_AREAS)
  })
})

describe('the overlay closes', () => {
  it('when the choice is answered, and the strip goes back between the boards', async () => {
    const { host, main, strip, game } = table('cards', PHONE)
    const user = userEvent.setup()
    const cards = [...host.querySelectorAll<HTMLElement>('button[data-candidate]')]
    const pending = must(game.session.store.getState().pending)
    const kind = pending.decision.kind
    const need = 'ChooseCards' in kind ? kind.ChooseCards.min : 0
    for (const card of cards.slice(0, need)) await user.click(card)
    await user.click(within(strip).getByRole('button', { name: /Confirm/ }))
    await act(async () => {})
    expect(host.hidden).toBe(true)
    expect(host.childElementCount).toBe(0)
    expect(main.style.gridTemplateAreas).toBe(TABLE_AREAS)
  })
})

describe('a wide window', () => {
  it('keeps a short choice of cards in the strip', () => {
    const { host, strip, main } = table('cards', WIDE)
    expect(host.hidden).toBe(true)
    expect(strip.querySelector('[data-slot="card-choices"]')).not.toBeNull()
    expect(main.style.gridTemplateAreas).toBe(TABLE_AREAS)
  })
})
