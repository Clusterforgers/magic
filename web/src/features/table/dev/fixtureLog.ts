/** About fifty log events for the fixture: one turn of an attack, seen from the viewer, in the order a game sends them. */
import type { LogEntry } from '@/play/store'
import type { Event, EventView, HiddenEvent, ObjectId, Step } from '@/protocol/generated'
import type { LogContext } from './fixture'

const pub = (event: Event): EventView => ({ Public: event })
const secret = (event: HiddenEvent): EventView => ({ Hidden: event })

// Ids of the first cards of a list, or nothing for a list that is too short.
const some = (ids: readonly ObjectId[], count: number): ObjectId[] => ids.slice(0, count)

/** The log of the fixture's turn, numbered from 0. */
export function buildLog(context: LogContext): LogEntry[] {
  const { names, viewer, active, defender, cards, attackers, blockers, stack } = context
  const others = names.map((_, seat) => seat).filter((seat) => seat !== active)
  const events: EventView[] = []
  const step = (name: Step): void => {
    events.push(pub({ StepBegan: { step: name } }))
  }
  const passAround = (): void => {
    events.push(pub({ PriorityGranted: { player: active } }), pub({ PriorityPassed: { player: active } }))
    for (const player of others.slice(0, 3)) events.push(pub({ PriorityPassed: { player } }))
  }

  events.push(pub({ TurnBegan: { number: 5, active } }))
  step('Untap')
  for (const object of some(cards[active]?.lands ?? [], 3))
    events.push(pub({ PermanentUntapped: { object } }))
  step('Upkeep')
  passAround()
  step('Draw')
  events.push(secret({ CardDrawn: { player: active } }), pub({ ManaPoolEmptied: { player: active } }))
  step('Main1')
  const lands = cards[active]?.lands ?? []
  const land = lands[5]
  if (land !== undefined) events.push(pub({ LandPlayed: { player: active, object: land } }))
  for (const object of some(lands.slice(6), 3)) {
    events.push(
      pub({ PermanentTapped: { object } }),
      pub({ ManaAdded: { player: active, mana: 'Green', source: object } }),
    )
  }
  const creature = cards[active]?.creatures.at(-1)
  if (creature !== undefined) {
    events.push(
      pub({ SpellCast: { player: active, object: creature } }),
      pub({ ManaSpent: { player: active, mana: ['Green', 'Green', 'Green'] } }),
    )
    passAround()
    events.push(pub({ SpellResolved: { object: creature } }))
  }
  events.push(
    pub({
      EffectCreated: {
        effect: {
          id: 1,
          layer: 'PtModify',
          selector: { Object: attackers[0] ?? 0 },
          modification: { ModifyPowerToughness: { power: 1, toughness: 1 } },
          duration: 'UntilEndOfTurn',
          timestamp: 1,
        },
      },
    }),
  )
  step('BeginCombat')
  step('DeclareAttackers')
  events.push(
    pub({
      AttackersDeclared: {
        player: active,
        attackers: attackers.map((id): [ObjectId, { Player: number }] => [id, { Player: defender }]),
      },
    }),
  )
  step('DeclareBlockers')
  const [firstAttacker] = attackers
  events.push(
    pub({
      BlockersDeclared: {
        player: defender,
        blocks:
          firstAttacker === undefined
            ? []
            : blockers.map((blocker): [ObjectId, ObjectId] => [blocker, firstAttacker]),
      },
    }),
  )
  for (const [index, object] of stack.entries()) {
    const caster = index === 0 ? (viewer + 2) % names.length : index === 1 ? active : viewer
    events.push(pub({ SpellCast: { player: caster, object } }), pub({ PriorityGranted: { player: caster } }))
  }
  events.push(
    pub({ PriorityPassed: { player: active } }),
    pub({ NothingToDo: { player: viewer, kind: 'Block' } }),
  )
  events.push(
    secret({ LibraryShuffled: { player: viewer } }),
    secret({ HandDealt: { player: viewer, count: 7 } }),
  )
  events.push(
    pub({
      DamageAssigned: {
        attacker: firstAttacker ?? 0,
        assignment: some(blockers, 2).map((id): [ObjectId, number] => [id, 2]),
      },
    }),
  )
  const [victim] = blockers
  if (victim !== undefined && firstAttacker !== undefined) {
    events.push(
      pub({ DamageDealt: { source: firstAttacker, target: { Object: victim }, amount: 3 } }),
      pub({ CreatureDied: { object: victim, cause: 'LethalDamage' } }),
    )
  }
  events.push(
    pub({ DamageDealt: { source: attackers[1] ?? 0, target: { Player: defender }, amount: 4 } }),
    pub('CombatEnded'),
  )
  events.push(pub('DamageCleared'), pub({ EffectExpired: { id: 1 } }), secret('ObjectLeftGame'))
  while (events.length < 50)
    events.push(pub({ PriorityPassed: { player: others[events.length % others.length] ?? active } }))
  return events.map((event, seq) => ({ seq, event }))
}
