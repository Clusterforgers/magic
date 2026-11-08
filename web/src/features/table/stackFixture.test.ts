// Goal: Soldier tokens made on different turns and given the same size and keywords by effects in a different order, as the engine projects them, stack once nothing a face shows tells them apart: the order of the keywords does not, summoning sickness, a tapped state and a part in combat do.
import { describe, expect, it } from 'vitest'
import type { CombatView, ObjectView } from '@/protocol/generated'
import fixtureJson from '@/protocol/fixtures/soldier-stacks.json'
import { combatRoleOf } from './cardModel'
import { groupPermanents } from './group'
import type { Groupable } from './group'

interface Stage {
  combat: CombatView
  objects: ObjectView[]
}
const fixture = fixtureJson as unknown as { tokens: number[]; attacking: Stage; idle: Stage }

/** The Soldier tokens of a stage as the table hands them to grouping, with the role combat gives each. */
function soldiers(stage: Stage): Groupable[] {
  return fixture.tokens.map((id) => {
    const object = stage.objects.find((candidate) => candidate.id === id)
    if (object === undefined) throw new Error(`token ${id} is missing`)
    return { ...object, role: combatRoleOf(stage.combat, id) }
  })
}

const ids = (stacks: ReturnType<typeof groupPermanents>): number[][] =>
  stacks.map((stack) => stack.members.map((member) => member.id))

describe('Soldier tokens from the engine', () => {
  it('are one stack when they look alike, whatever order their keywords were granted in', () => {
    const [first, second, third, fourth] = fixture.tokens
    const tokens = soldiers(fixture.idle)
    expect(tokens.map((token) => token.keywords)).toContainEqual(['Lifelink', 'Vigilance'])
    expect(tokens.map((token) => token.keywords)).toContainEqual(['Vigilance', 'Lifelink'])
    expect(ids(groupPermanents(tokens))).toEqual([[first, second, third, fourth]])
  })

  it('keep the sick and tapped ones apart and every attacker alone', () => {
    const [first, second, third, fourth] = fixture.tokens
    const tokens = soldiers(fixture.attacking)
    expect(tokens.map((token) => token.role)).toEqual([null, null, 'attacking', 'attacking'])
    expect(ids(groupPermanents(tokens))).toEqual([[first, second], [third], [fourth]])
  })
})
