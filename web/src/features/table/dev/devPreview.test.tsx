// Goal: the dev table answers a preview the way the engine would, a moment after the question: the forecast of the combat with the blocks, or the attack, declared (from the cards of the fixture), so that `?prompt=block` and `?prompt=attack` show the whole flow of "Checking outcome…" and then the outcome; no attackers and any other answer get none, and only the answer to the latest question counts.
import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { GameContext } from '@/features/game/gameContext'
import type { Answer, AttackTarget, ObjectId } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { DEBOUNCE_MS } from '../combat/usePreviewRequest'
import { GameTable } from '../GameTable'
import { devForecast, devPreview } from './devForecast'
import { fixtureGame } from './devGame'

/** The dev block prompt: the viewer's blockers, the attackers they may block and the table to look at. */
function blockGame(seats = 3) {
  const game = fixtureGame({ seats, prompt: 'block' })
  const state = game.session.store.getState()
  const view = must(state.view)
  const kind = must(state.pending).decision.kind
  const options = must('DeclareBlockers' in kind ? kind.DeclareBlockers : null)
  const blockers = options.blockers.map((option) => option.object)
  const attackers = must(options.blockers[0]).can_block
  return { game, view, blockers, attackers }
}

describe('devPreview', () => {
  it('says what the table says for no blocks at all', () => {
    const { view } = blockGame()
    expect(devPreview(view, { DeclareBlockers: [] })).toEqual(devForecast(view))
  })

  it('declares the blocks of the answer on top of any other defender made, in the order of the answer', () => {
    const { view, blockers, attackers } = blockGame()
    const [first, second] = blockers as [ObjectId, ObjectId]
    const [target, other] = attackers as [ObjectId, ObjectId]
    const forecast = must(
      devPreview(view, {
        DeclareBlockers: [
          [second, target],
          [first, target],
          [blockers[2] ?? first, other],
        ],
      }),
    )
    const blocked = must(forecast.attackers.find((entry) => entry.attacker === target))
    expect(blocked).toMatchObject({ blocked: true, blockers: [second, first] })
    expect(blocked.receives.map((hit) => hit.source)).toEqual([second, first])
    expect(forecast.attackers.find((entry) => entry.attacker === other)?.blockers).toHaveLength(1)
    // The attackers that nothing blocks still deal their damage to the player.
    const unblocked = forecast.attackers.filter((entry) => !entry.blocked)
    expect(unblocked.length).toBeGreaterThan(0)
    expect(unblocked.every((entry) => entry.deals.every((hit) => 'Player' in hit.target))).toBe(true)
  })

  it('forecasts the attack of the answer as unblocked, creature by creature', () => {
    const game = fixtureGame({ seats: 3, prompt: 'attack' })
    const state = game.session.store.getState()
    const kind = must(state.pending).decision.kind
    const options = must('DeclareAttackers' in kind ? kind.DeclareAttackers : null)
    const [a, b] = options.attackers.map((option) => option.object) as [ObjectId, ObjectId]
    const target: AttackTarget = { Player: 1 }
    const forecast = must(
      devPreview(must(state.view), {
        DeclareAttackers: [
          [a, target],
          [b, target],
        ],
      }),
    )
    expect(forecast.attackers.map((entry) => [entry.attacker, entry.blocked])).toEqual([
      [a, false],
      [b, false],
    ])
    expect(forecast.players.map((player) => player.player)).toEqual([1])
    expect(must(forecast.players[0]).damage).toBeGreaterThan(0)
  })

  it.each<[string, Answer]>([
    ['an attack with nobody', { DeclareAttackers: [] }],
    ['a pass', { Priority: 'Pass' }],
    ['a discard', { Discard: [1] }],
  ])('has no forecast for %s', (_what, answer) => {
    expect(devPreview(blockGame().view, answer)).toBeNull()
  })
})

describe('the dev table', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  function mount() {
    const t = blockGame()
    const { container } = render(
      <GameContext value={t.game}>
        <GameTable />
      </GameContext>,
    )
    const card = (id: ObjectId): HTMLElement =>
      must(container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`), `card ${id}`)
    const state = (): string | null =>
      container.querySelector('[data-slot="block-outcome"]')?.getAttribute('data-state') ?? null
    const wait = (ms: number): void => void act(() => vi.advanceTimersByTime(ms))
    const block = (blocker: ObjectId, attacker: ObjectId): void => {
      fireEvent.click(card(blocker))
      fireEvent.click(card(attacker))
    }
    const doomed = (): number => container.querySelectorAll('button[data-doomed]').length
    return { ...t, container, state, wait, block, doomed, store: t.game.session.store }
  }

  it('says it is checking, then answers with the outcome of the blocks chosen', () => {
    const t = mount()
    const [blocker] = t.blockers as [ObjectId]
    const [attacker] = t.attackers as [ObjectId]
    expect(t.state()).toBe('ready')
    t.block(blocker, attacker)
    expect(t.state()).toBe('checking')
    t.wait(DEBOUNCE_MS)
    expect(t.state()).toBe('checking')
    t.wait(200)
    expect(t.state()).toBe('ready')
    const preview = must(t.store.getState().preview)
    expect(preview.forecast?.attackers.find((entry) => entry.attacker === attacker)?.blockers).toContain(
      blocker,
    )
  })

  it('counts only the answer to the latest question', () => {
    const t = mount()
    const [first, second] = t.blockers as [ObjectId, ObjectId]
    const [attacker] = t.attackers as [ObjectId]
    t.block(first, attacker)
    t.wait(DEBOUNCE_MS)
    // The first answer is on its way when the choice changes.
    t.block(second, attacker)
    t.wait(DEBOUNCE_MS)
    expect(t.state()).toBe('checking')
    t.wait(200)
    expect(t.state()).toBe('ready')
    expect(
      t.store.getState().preview?.forecast?.attackers.find((entry) => entry.attacker === attacker)?.blockers,
    ).toEqual([first, second])
  })

  it('answers nothing once the decision is answered', () => {
    const t = mount()
    const [blocker] = t.blockers as [ObjectId]
    const [attacker] = t.attackers as [ObjectId]
    t.block(blocker, attacker)
    t.wait(DEBOUNCE_MS)
    fireEvent.click(screen.getByRole('button', { name: 'Confirm blocks' }))
    expect(t.store.getState().pending).toBeNull()
    t.wait(500)
    expect(t.store.getState().preview).toBeNull()
  })
})
