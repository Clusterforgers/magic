// Goal: the keys of a decision work in its home: A moves the focus to the answers, Enter with nothing focused presses the primary answer, Escape hides a window before it cancels, a Tab ring runs answers → detail → lit objects and round; and a player target is pressed on its pod, which wears the legal mark, while a pointed stack item lights its source and targets.
import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import type { Answer } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { fixtureGame } from '../dev/devGame'
import { GameTable } from '../GameTable'
import { TableOptionsContext } from '../tableOptions'
import { nextInRing } from './homeKeys'

function table(prompt: string, viewport = { width: 1400, height: 900 }, seats = 2) {
  const game = fixtureGame({ seats, board: seats === 2 ? 'duel' : undefined, prompt })
  const answer = vi.fn((_answer: Answer) => true)
  render(
    <GameContext value={{ ...game, session: { ...game.session, answer } }}>
      <TableOptionsContext value={{ viewport }}>
        <GameTable />
      </TableOptionsContext>
    </GameContext>,
  )
  return { answer, user: userEvent.setup(), game }
}

describe('nextInRing', () => {
  it.each([
    [['a', 'b', 'c'], 'a', false, 'b'],
    [['a', 'b', 'c'], 'c', false, 'a'],
    [['a', 'b', 'c'], 'a', true, 'c'],
    [['a', 'b', 'c'], 'x', false, null],
    [[], 'a', false, null],
  ] as const)('in %j from %s (back: %s) is %s', (ring, current, back, next) => {
    expect(nextInRing(ring, current, back)).toBe(next)
  })
})

describe('the keys of a decision', () => {
  it('moves the focus to the answers with A, and presses the primary with Enter when nothing is focused', async () => {
    const { answer, user } = table('ask-may')
    await user.keyboard('a')
    const primary = must(document.querySelector('[data-coach="ask.primary"]'))
    expect(primary).toHaveFocus()
    act(() => (document.activeElement as HTMLElement).blur())
    await user.keyboard('{Enter}')
    expect(answer).toHaveBeenCalledTimes(1)
  })

  it('hides a window with Escape before Escape cancels', async () => {
    const { answer, user } = table('mode-two', { width: 1100, height: 800 })
    const window = (): HTMLElement =>
      must(document.querySelector<HTMLElement>('[data-slot="decision-window"]'))
    await user.keyboard('{Escape}')
    expect(window()).toHaveAttribute('data-peek')
    expect(answer).not.toHaveBeenCalled()
    await user.keyboard('{Escape}')
    expect(answer).toHaveBeenCalledWith('Undo')
  })

  it('runs Tab from the answers to the lit cards of the table and round during a board choice', async () => {
    const { user } = table('targets-two')
    await user.keyboard('a')
    const lit = [...document.querySelectorAll<HTMLElement>('[data-area] button[data-highlight="legal"]')]
    expect(lit.length).toBeGreaterThan(0)
    for (let step = 0; step < 40 && !lit.includes(document.activeElement as HTMLElement); step += 1)
      await user.keyboard('{Tab}')
    expect(lit).toContain(document.activeElement)
  })
})

describe('a player target on the table', () => {
  it('is pressed on its pod, which wears the legal mark, and answers at once for one target', async () => {
    const { answer, user, game } = table('targets', { width: 1400, height: 900 }, 4)
    const pending = must(game.session.store.getState().pending)
    const legal = (pending.decision.kind as unknown as { ChooseTargets: { legal: Answer[] } }).ChooseTargets
      .legal
    const seat = legal.flatMap((target) =>
      typeof target === 'object' && target !== null && 'Player' in target ? [target.Player as number] : [],
    )[0]
    const pod = must(document.querySelector<HTMLElement>(`[data-seat="${must(seat)}"][data-defender]`))
    expect(document.querySelector('[data-board-choice="ChooseTargets"]')).not.toBeNull()
    await user.click(pod)
    expect(answer).toHaveBeenCalledWith({ ChooseTargets: { Targets: [{ Player: seat }] } })
  })
})

describe('a pointed stack item', () => {
  it('lights its targets on the table and shows its preview while focused or hovered', () => {
    let creature = 0
    renderTable({
      seats: 4,
      state: (state) => {
        const view = clone(must(state.view))
        creature = must(view.objects.find((o) => o.zone === 'Battlefield' && o.types.includes('Creature'))).id
        const top = must(view.objects.find((o) => o.id === view.stack.cards?.at(-1)))
        top.targets = [[{ Object: { id: creature, counter: 0 } }]]
        return { ...state, view, objectsById: Object.fromEntries(view.objects.map((o) => [o.id, o])) }
      },
    })
    const marks = (): string => document.querySelector('[data-slot="ask-marks"]')?.textContent ?? ''
    expect(marks()).not.toContain(`[data-object-id='${creature}']`)
    fireEvent.focus(must(document.querySelector<HTMLElement>('[data-stack-chip]')))
    expect(marks()).toContain(`button[data-object-id='${creature}'][data-size]{outline:3px solid`)
    expect(screen.queryByTestId('card-preview')).not.toBeNull()
  })
})
