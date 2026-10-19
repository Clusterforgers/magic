/** Goal: `?prompt=targets|targets-two|targets-trigger|order` and the two on a clock hand the viewer, at every seat count, a decision the wire parser accepts and whose players and cards are on the table, a spell that can be taken back and a triggered ability that cannot, and each can be drawn and answered on the dev table, which clears it. */
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import { GameTable } from '@/features/table/GameTable'
import { TableOptionsContext } from '@/features/table/tableOptions'
import { parseMessage } from '@/net/parse'
import { must } from '@/test-utils/batches'
import { fixtureGame } from './devGame'

const TARGETS = ['targets', 'targets-two', 'targets-trigger', 'targets-clock'] as const
const ORDERS = ['order', 'order-clock'] as const

describe.each(TARGETS)('?prompt=%s', (prompt) => {
  it.each([2, 4, 12])('faces a target decision of real players and cards at %i seats', (seats) => {
    const { pending, view } = fixtureGame({ seats, prompt }).session.store.getState()
    const { decision, id } = must(pending)
    const kind = decision.kind
    if (!('ChooseTargets' in kind)) throw new Error('not a target decision')
    const options = kind.ChooseTargets
    expect(decision.player).toBe(0)
    expect(options.legal.length).toBeGreaterThan(0)
    const ids = new Set(must(view).objects.map((o) => o.id))
    expect(ids.has(options.source)).toBe(true)
    for (const target of options.legal) {
      if ('Player' in target) expect(target.Player).toBeLessThan(seats)
      else expect(ids.has(target.Object.id)).toBe(true)
    }
    expect(options.max).toBeLessThanOrEqual(options.legal.length)
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
    expect(document.querySelector('[data-decision="ChooseTargets"]')).not.toBeNull()
    const group = screen.getByRole('group', { name: 'Legal targets' })
    if (prompt !== 'targets-two') await user.click(must(within(group).getAllByRole('button')[0]))
    await user.click(screen.getByRole('button', { name: 'Confirm targets' }))
    expect(document.querySelector('[data-decision]')).toBeNull()
    expect(game.session.store.getState().pending).toBeNull()
  })
})

describe('the target decisions', () => {
  it('give a spell a way out and a triggered ability none', () => {
    const spell = fixtureGame({ seats: 2, prompt: 'targets' }).session.store.getState().pending
    const trigger = fixtureGame({ seats: 2, prompt: 'targets-trigger' }).session.store.getState().pending
    const options = (kind: object | undefined) =>
      kind !== undefined && 'ChooseTargets' in kind
        ? (kind.ChooseTargets as { can_cancel: boolean; triggered?: boolean })
        : undefined
    expect(options(spell?.decision.kind)?.can_cancel).toBe(true)
    expect(options(trigger?.decision.kind)).toMatchObject({ can_cancel: false, triggered: true })
  })

  it('run on a response clock when asked to', () => {
    for (const prompt of ['targets-clock', 'order-clock']) {
      expect(fixtureGame({ seats: 2, prompt }).session.store.getState().clock).not.toBeNull()
    }
    expect(fixtureGame({ seats: 2, prompt: 'targets' }).session.store.getState().clock).toBeNull()
  })
})

describe.each(ORDERS)('?prompt=%s', (prompt) => {
  it.each([2, 4, 12])('faces an order decision of real triggers at %i seats', (seats) => {
    const { pending, view } = fixtureGame({ seats, prompt }).session.store.getState()
    const { decision, id } = must(pending)
    const kind = decision.kind
    if (!('OrderTriggers' in kind)) throw new Error('not an order decision')
    const ids = new Set(must(view).objects.map((o) => o.id))
    expect(kind.OrderTriggers.triggers.length).toBeGreaterThanOrEqual(2)
    for (const trigger of kind.OrderTriggers.triggers) expect(ids.has(trigger.source.id)).toBe(true)
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
    expect(document.querySelector('[data-decision="OrderTriggers"]')).not.toBeNull()
    await user.click(screen.getByRole('button', { name: 'Confirm order' }))
    expect(document.querySelector('[data-decision]')).toBeNull()
    expect(game.session.store.getState().pending).toBeNull()
  })
})
