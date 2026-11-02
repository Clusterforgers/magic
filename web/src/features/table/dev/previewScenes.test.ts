/** Goal: `?prompt=priority-stack-ability`, `priority-companion` and `attack-battle` give the dev table an ability on the stack, a companion at every stage and a battle with its protector that the viewer may attack, at every seat count. */
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import { fixtureGame } from './devGame'

const stateOf = (prompt: string, seats: number) => {
  const { pending, view } = fixtureGame({ seats, prompt }).session.store.getState()
  return { pending: must(pending), view: must(view) }
}

describe.each([2, 4, 12])('at %i seats', (seats) => {
  it('stacks an ability of a creature on top, with its words and no cost', () => {
    const { view } = stateOf('priority-stack-ability', seats)
    const top = view.objects.find((o) => o.id === view.stack.cards?.at(-1))
    expect(top?.kind).toBe('Ability')
    expect(top?.mana_cost).toBeNull()
    expect(top?.rules_text?.length).toBeGreaterThan(80)
    expect(view.objects.some((o) => o.id === top?.source)).toBe(true)
  })

  it('names a companion at each stage, one of them without a name', () => {
    const { view } = stateOf('priority-companion', seats)
    const stages = view.players.map((p) => p.companion?.stage)
    expect(stages.every((stage) => stage !== undefined)).toBe(true)
    expect(new Set(stages).size).toBe(Math.min(3, seats))
    if (seats >= 2) expect(view.players[1]?.companion?.name).toBeUndefined()
  })

  it('offers the battle, protected by another seat, as a target of every attacker', () => {
    const { pending, view } = stateOf('attack-battle', seats)
    const battle = must(view.objects.find((o) => o.types.includes('Battle')))
    expect(battle.protector).not.toBeUndefined()
    expect(battle.protector).not.toBe(battle.controller)
    const kind = pending.decision.kind
    if (!('DeclareAttackers' in kind)) throw new Error('not an attack')
    const { attackers } = kind.DeclareAttackers
    expect(attackers.length).toBeGreaterThan(0)
    for (const attacker of attackers) {
      expect(attacker.defenders).toContainEqual({ Planeswalker: battle.id })
    }
  })
})
