// Goal: the dev table can hand the viewer an attack or a block to declare at every seat count (`prompt=attack|block`): a valid table at the right moment of the turn (the viewer's own declare-attackers step with a planeswalker to attack, or six attackers in the declare-blockers step), options made of real cards that a realistic engine would offer (every other seat and the planeswalker as defenders, only attackers aimed at the viewer to block), a wire-valid decision, and a prompt that can be used on the real table and answered.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { isStateView, parseMessage } from '@/net/parse'
import { GameContext } from '@/features/game/gameContext'
import type { AttackTarget, ObjectId, StateView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { fixtureGame } from './dev/devGame'
import { GameTable } from './GameTable'

const COUNTS = [2, 3, 4, 8, 12]

const decisionOf = (seats: number, prompt: string) => {
  const game = fixtureGame({ seats, prompt })
  const { pending, view, objectsById } = game.session.store.getState()
  return { game, pending: must(pending), view: must(view), objectsById }
}

describe('prompt=attack', () => {
  it.each(COUNTS)('at %i seats is the viewer declaring attackers, with nothing declared yet', (seats) => {
    const { view } = decisionOf(seats, 'attack')
    expect(view.turn).toMatchObject({ active: 0, step: 'DeclareAttackers' })
    expect(view.combat).toEqual({ attackers: [], attackers_declared: false, blockers_declared: [] })
    expect(view.priority).toBeNull()
    expect(isStateView(view)).toBe(true)
  })

  it.each(COUNTS)(
    'at %i seats offers untapped creatures that may attack every other seat and a planeswalker',
    (seats) => {
      const { pending, view, objectsById } = decisionOf(seats, 'attack')
      const kind = pending.decision.kind
      const options = must('DeclareAttackers' in kind ? kind.DeclareAttackers : null)
      expect(options.attackers.length).toBeGreaterThanOrEqual(4)
      for (const { object, defenders } of options.attackers) {
        const creature = must(objectsById[object])
        expect(creature).toMatchObject({ controller: 0, zone: 'Battlefield' })
        expect(creature.types).toContain('Creature')
        expect(creature.permanent).toMatchObject({ tapped: false, summoning_sick: false })
        expect(defenders.filter((target) => 'Player' in target)).toEqual(
          Array.from({ length: seats - 1 }, (_, index): AttackTarget => ({ Player: index + 1 })),
        )
        const walkers = defenders.flatMap((target) => ('Planeswalker' in target ? [target.Planeswalker] : []))
        expect(walkers).toHaveLength(1)
        expect(objectsById[must(walkers[0])]).toMatchObject({ controller: 1, zone: 'Battlefield' })
      }
      const walker = must(view.objects.find((object) => object.types.includes('Planeswalker')))
      expect(must(view.battlefield.cards)).toContain(walker.id)
      expect(view.battlefield.count).toBe(view.battlefield.cards?.length)
      // Some creatures are tapped or summoning sick, so dimmed ones can be seen.
      const mine = view.objects.filter(
        (object) =>
          object.controller === 0 && object.zone === 'Battlefield' && object.types.includes('Creature'),
      )
      expect(mine.length).toBeGreaterThan(options.attackers.length)
    },
  )

  it('is a decision message the wire parser accepts', () => {
    const { pending } = decisionOf(4, 'attack')
    expect(parseMessage({ type: 'Decision', id: pending.id, decision: pending.decision }).ok).toBe(true)
  })
})

describe('prompt=block', () => {
  it.each(COUNTS)(
    'at %i seats is the declare-blockers step with six attackers and no block by the viewer yet',
    (seats) => {
      const { view } = decisionOf(seats, 'block')
      expect(view.turn.step).toBe('DeclareBlockers')
      expect(view.combat.attackers).toHaveLength(6)
      expect(view.combat.attackers_declared).toBe(true)
      expect(view.combat.blockers_declared).not.toContain(0)
      for (const attacker of view.combat.attackers) {
        const blockers = attacker.blockers.map((id) => view.objects.find((object) => object.id === id))
        // The only blocks are those of another defender, who has double-blocked one attacker aimed at them.
        for (const blocker of blockers) expect(blocker?.controller).not.toBe(0)
        expect(attacker.blocked).toBe(attacker.blockers.length > 0)
        expect(view.objects.find((object) => object.id === attacker.object)).toMatchObject({
          controller: view.turn.active,
          permanent: { tapped: true },
        })
      }
      expect(view.combat.attackers.filter((attacker) => attacker.blocked)).toHaveLength(seats === 2 ? 0 : 1)
      expect(isStateView(view)).toBe(true)
    },
  )

  it.each(COUNTS)(
    'at %i seats offers every untapped creature a block on each attacker aimed at the viewer',
    (seats) => {
      const { pending, view, objectsById } = decisionOf(seats, 'block')
      const kind = pending.decision.kind
      const options = must('DeclareBlockers' in kind ? kind.DeclareBlockers : null)
      const aimed = view.combat.attackers
        .filter((attacker) => 'Player' in attacker.target && attacker.target.Player === 0)
        .map((attacker) => attacker.object)
      // Four attackers at the viewer, and with a third seat two more at another player.
      expect(aimed).toHaveLength(seats === 2 ? 6 : 4)
      expect(options.blockers.length).toBeGreaterThanOrEqual(4)
      for (const { object, can_block } of options.blockers) {
        expect(objectsById[object]).toMatchObject({ controller: 0, zone: 'Battlefield' })
        expect(objectsById[object]?.permanent?.tapped).toBe(false)
        expect(can_block).toEqual(aimed)
      }
      const others = view.combat.attackers.filter((attacker) => !aimed.includes(attacker.object))
      for (const attacker of others) expect(attacker.target).not.toEqual({ Player: 0 })
    },
  )

  it('is a decision message the wire parser accepts', () => {
    const { pending } = decisionOf(12, 'block')
    expect(parseMessage({ type: 'Decision', id: pending.id, decision: pending.decision }).ok).toBe(true)
  })
})

describe.each([
  ['attack', 'DeclareAttackers', 'Attack with all', 'Confirm attack'],
  ['block', 'DeclareBlockers', null, 'No blocks'],
] as const)('the %s prompt on the table', (prompt, decision, first, last) => {
  it.each([2, 4, 12])('is drawn at %i seats with legal and dimmed cards, and answers once', async (seats) => {
    const game = fixtureGame({ seats, prompt })
    const user = userEvent.setup()
    const { container } = render(
      <GameContext value={game}>
        <GameTable />
      </GameContext>,
    )
    expect(container.querySelector(`[data-decision="${decision}"]`)).not.toBeNull()
    expect(container.querySelectorAll('[data-highlight="legal"]').length).toBeGreaterThanOrEqual(4)
    expect(container.querySelectorAll('[data-highlight="dimmed"]').length).toBeGreaterThan(3)
    if (first !== null) await user.click(screen.getByRole('button', { name: first }))
    await user.click(screen.getByRole('button', { name: last }))
    expect(game.session.store.getState().pending).toBeNull()
    expect(container.querySelector('[data-decision]')).toBeNull()
  })
})

describe('the other prompts of the dev table', () => {
  it('leave the fixture table as it is', () => {
    const plain = fixtureGame({ seats: 3 }).session.store.getState().view as StateView
    const priority = fixtureGame({ seats: 3, prompt: 'priority' }).session.store.getState().view as StateView
    expect(priority.combat).toEqual(plain.combat)
    expect(priority.turn).toEqual(plain.turn)
    const ids: ObjectId[] = priority.objects.map((object) => object.id)
    expect(ids).toEqual(plain.objects.map((object) => object.id))
  })
})
