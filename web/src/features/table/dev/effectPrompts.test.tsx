/** Goal: `?prompt=targets|targets-two|targets-trigger|order|yesno|option|option-players|mode|mode-two|mode-trigger` and those on a clock hand the viewer, at every seat count, a decision the wire parser accepts and whose players and cards are on the table, a spell that can be taken back and a triggered ability that cannot, and each can be drawn and answered on the dev table, which clears it. */
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

/** The prompt names of each question kind. */
const QUESTIONS = [
  { kind: 'YesNo', prompts: ['yesno', 'yesno-clock'] },
  { kind: 'ChooseOption', prompts: ['option', 'option-players', 'option-clock'] },
  { kind: 'ChooseMode', prompts: ['mode', 'mode-two', 'mode-trigger', 'mode-clock'] },
] as const

/** Answers the question in `line` as a person would: Yes, the first option or mode, or two modes and Confirm modes. */
async function answerQuestion(user: ReturnType<typeof userEvent.setup>, line: HTMLElement, prompt: string) {
  if (prompt.startsWith('yesno')) return user.click(within(line).getByRole('button', { name: 'Yes' }))
  const choices = within(line)
    .getAllByRole('button')
    .filter((b) => b.hasAttribute('aria-keyshortcuts'))
  await user.click(must(choices[0]))
  if (prompt !== 'mode-two') return
  await user.click(must(choices[1]))
  await user.click(within(line).getByRole('button', { name: 'Confirm modes' }))
}

describe.each(QUESTIONS.flatMap(({ kind, prompts }) => prompts.map((prompt) => ({ kind, prompt }))))(
  '?prompt=$prompt',
  ({ kind, prompt }) => {
    it.each([2, 4, 12])(
      'faces a %s decision whose source and players are on the table at %i seats',
      (seats) => {
        const { pending, view } = fixtureGame({ seats, prompt }).session.store.getState()
        const { decision, id } = must(pending)
        expect(kind in decision.kind).toBe(true)
        expect(decision.player).toBe(0)
        const ids = new Set(must(view).objects.map((o) => o.id))
        const options = (decision.kind as Record<string, Record<string, unknown>>)[kind] ?? {}
        expect(ids.has(options.source as number)).toBe(true)
        for (const entry of (options.options as { Player?: number }[] | undefined) ?? []) {
          if (entry.Player !== undefined) expect(entry.Player).toBeLessThan(seats)
        }
        expect(parseMessage({ type: 'Decision', id, decision }).ok).toBe(true)
      },
    )

    it('is drawn on the table and answered by a button, which clears it', async () => {
      const game = fixtureGame({ seats: 2, prompt })
      render(
        <GameContext value={game}>
          <TableOptionsContext value={{}}>
            <GameTable />
          </TableOptionsContext>
        </GameContext>,
      )
      const user = userEvent.setup()
      await answerQuestion(
        user,
        must(document.querySelector<HTMLElement>(`[data-decision="${kind}"]`)),
        prompt,
      )
      expect(document.querySelector('[data-decision]')).toBeNull()
      expect(game.session.store.getState().pending).toBeNull()
    })

    it('runs on a response clock only when asked to', () => {
      const clock = fixtureGame({ seats: 2, prompt }).session.store.getState().clock
      expect(clock !== null).toBe(prompt.endsWith('-clock'))
    })
  },
)

describe('the mode decisions', () => {
  it('give a spell a way out and a triggered ability none', () => {
    const can = (prompt: string): boolean | undefined => {
      const kind = fixtureGame({ seats: 2, prompt }).session.store.getState().pending?.decision.kind
      return kind !== undefined && 'ChooseMode' in kind ? kind.ChooseMode.can_cancel : undefined
    }
    expect([can('mode'), can('mode-two'), can('mode-trigger')]).toEqual([true, true, false])
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
