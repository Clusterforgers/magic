/** Goal: `?prompt=cards|cards-battlefield|cards-many|order-cards` and those on a clock hand the viewer, at every seat count, a decision the wire parser accepts whose source and cards are on the table (a hand, creatures of the battlefield, forty cards of a library), a long list gets a filter box and a short one none, and each can be drawn and answered on the dev table, which clears it. */
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { parseMessage } from '@/net/parse'
import { must } from '@/test-utils/batches'
import { fixtureGame } from './devGame'

/** The prompt names of the card decisions, each also on a clock. */
const CARDS = ['cards', 'cards-battlefield', 'cards-many', 'order-cards'] as const

function table(prompt: string) {
  const game = fixtureGame({ seats: 2, prompt })
  const view = render(
    <GameContext value={game}>
      <TableOptionsContext value={{}}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  return { game, ...view }
}

describe.each(CARDS.flatMap((prompt) => [prompt, `${prompt}-clock`]))('?prompt=%s', (prompt) => {
  const ordering = prompt.startsWith('order-cards')
  const name = ordering ? 'OrderCards' : 'ChooseCards'

  it.each([2, 4, 12])('faces a decision of real cards of the table at %i seats', (seats) => {
    const { pending, view } = fixtureGame({ seats, prompt }).session.store.getState()
    const { decision, id } = must(pending)
    expect(decision.player).toBe(0)
    const ids = new Set(must(view).objects.map((o) => o.id))
    const kind = decision.kind
    const options = 'OrderCards' in kind ? kind.OrderCards : 'ChooseCards' in kind ? kind.ChooseCards : null
    if (options === null) throw new Error('not a card decision')
    expect(name in kind).toBe(true)
    expect(ids.has(options.source)).toBe(true)
    const cards = 'cards' in options ? options.cards : options.from
    expect(cards.length).toBeGreaterThan(0)
    for (const card of cards) expect(ids.has(card)).toBe(true)
    if ('from' in options) expect(options.min).toBeLessThanOrEqual(options.max)
    expect(options.text).toBeTruthy()
    expect(parseMessage({ type: 'Decision', id, decision }).ok).toBe(true)
  })

  it('is drawn on the table and answered by its buttons, which clears it', async () => {
    const { game } = table(prompt)
    const user = userEvent.setup()
    expect(document.querySelector(`[data-decision="${name}"]`)).not.toBeNull()
    if (ordering) {
      await user.click(screen.getByRole('button', { name: 'Confirm order' }))
    } else {
      const group = screen.getByRole('group', { name: 'Cards to choose from' })
      const wanted = prompt.startsWith('cards-many') ? 0 : prompt.startsWith('cards-battlefield') ? 1 : 2
      for (const card of within(group).getAllByRole('button').slice(0, wanted)) await user.click(card)
      await user.click(screen.getByRole('button', { name: 'Confirm' }))
    }
    expect(document.querySelector('[data-decision]')).toBeNull()
    expect(game.session.store.getState().pending).toBeNull()
  })

  it('runs on a response clock only when asked to', () => {
    const clock = fixtureGame({ seats: 2, prompt }).session.store.getState().clock
    expect(clock !== null).toBe(prompt.endsWith('-clock'))
  })
})

describe('the card decisions', () => {
  const cardsOf = (prompt: string): number[] => {
    const kind = fixtureGame({ seats: 2, prompt }).session.store.getState().pending?.decision.kind
    return kind !== undefined && 'ChooseCards' in kind ? kind.ChooseCards.from : []
  }

  it('offer the viewer their hand, four creatures of their battlefield and forty cards of their library', () => {
    const { view } = fixtureGame({ seats: 2, prompt: 'cards-many' }).session.store.getState()
    const zoneOf = (id: number): string =>
      JSON.stringify(must(must(view).objects.find((o) => o.id === id)).zone)
    expect(cardsOf('cards')).toHaveLength(7)
    expect(cardsOf('cards-battlefield').map(zoneOf)).toEqual(Array(4).fill('"Battlefield"'))
    expect(cardsOf('cards-many')).toHaveLength(40)
    expect(cardsOf('cards-many').every((id) => zoneOf(id) === '{"Library":0}')).toBe(true)
  })

  it.each([
    ['cards-many', true],
    ['cards', false],
    ['cards-battlefield', false],
  ])('on ?prompt=%s have a filter box: %s', (prompt, filter) => {
    table(prompt)
    expect(screen.queryByRole('searchbox', { name: 'Filter cards' }) !== null).toBe(filter)
  })
})
