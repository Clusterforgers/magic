/**
 * The dev table's `?fx=strike`: a whole combat in one batch, so its combat damage strikes a player, a planeswalker and a
 * creature. Up to three of the viewer's creatures outside the combat attack: the first the opponent, the second the
 * opponent's planeswalker (the opponent when there is none), the third is blocked by an opponent's creature and the
 * two trade blows.
 */
import type { AttackTarget, Event, ObjectId, ObjectView, PlayerId } from '@/protocol/generated'
import type { PlayScene } from './playScenes'

const creatures = (scene: PlayScene, seat: PlayerId): ObjectView[] =>
  scene.next.objects.filter(
    (object) =>
      object.zone === 'Battlefield' &&
      object.controller === seat &&
      object.types.includes('Creature') &&
      object.permanent !== null,
  )

const power = (object: ObjectView): number => Math.max(1, object.power ?? 1)

/** The damage a creature can take and live. */
const room = (object: ObjectView): number =>
  Math.max(0, (object.toughness ?? 2) - (object.permanent?.damage ?? 0) - 1)

/** The opponent's planeswalker; one joins the battlefield, made from `like`, when the opponent has none. */
function walkerOf(scene: PlayScene, like: ObjectView | undefined): ObjectView | undefined {
  const { next, foe } = scene
  const found = next.objects.find(
    (object) =>
      object.zone === 'Battlefield' && object.controller === foe && object.types.includes('Planeswalker'),
  )
  if (found !== undefined || like === undefined) return found
  const id = Math.max(0, ...next.objects.map((object) => object.id)) + 1
  const walker: ObjectView = {
    ...structuredClone(like),
    id,
    name: 'Jace Beleren',
    types: ['Planeswalker'],
    subtypes: ['Jace'],
    power: null,
    toughness: null,
    counters: [['loyalty', 5]],
    permanent: { tapped: false, damage: 0, summoning_sick: false },
  }
  scene.add(walker)
  next.battlefield = { count: next.battlefield.count + 1, cards: [...(next.battlefield.cards ?? []), id] }
  return walker
}

/** The batch's events; the scene's table is left after combat damage with the damage marked and the life lost. */
function strike(scene: PlayScene): Event[] {
  const { next, viewer, foe } = scene
  const busy = new Set(next.combat.attackers.flatMap((attacker) => [attacker.object, ...attacker.blockers]))
  const attackers = creatures(scene, viewer)
    .filter((object) => !busy.has(object.id))
    .slice(0, 3)
  const [blocker] = creatures(scene, foe).filter((object) => !busy.has(object.id))
  if (attackers.length === 0) return []
  const walker = walkerOf(scene, blocker)
  const targets = attackers.map((_, index): AttackTarget =>
    index === 1 && walker !== undefined ? { Planeswalker: walker.id } : { Player: foe },
  )
  const blocked = attackers.length === 3 && blocker !== undefined ? attackers[2] : undefined
  next.combat = {
    attackers: attackers.map((object, index) => ({
      object: object.id,
      target: targets[index] ?? { Player: foe },
      blocked: object === blocked,
      blockers: object === blocked && blocker !== undefined ? [blocker.id] : [],
    })),
    attackers_declared: true,
    blockers_declared: blocker === undefined ? [] : [foe],
  }
  next.turn = { ...next.turn, step: 'CombatDamage' }
  next.priority = { holder: next.turn.active, passed: [] }
  const events: Event[] = [
    {
      AttackersDeclared: {
        player: viewer,
        attackers: attackers.map((object, index): [ObjectId, AttackTarget] => [
          object.id,
          targets[index] ?? { Player: foe },
        ]),
      },
    },
  ]
  if (blocked !== undefined && blocker !== undefined)
    events.push({ BlockersDeclared: { player: foe, blocks: [[blocker.id, blocked.id]] } })
  events.push({ StepBegan: { step: 'CombatDamage' } })
  const mark = (id: ObjectId, amount: number): void =>
    scene.edit(id, (object) => {
      if (object.permanent !== null) object.permanent.damage += amount
    })
  for (const [index, attacker] of attackers.entries()) {
    scene.edit(attacker.id, (object) => {
      if (object.permanent !== null) object.permanent.tapped = true
    })
    const amount = power(attacker)
    const target = targets[index] ?? { Player: foe }
    if (attacker === blocked && blocker !== undefined) {
      // Both live, so the strike and its impact stay on the table to be looked at.
      const into = Math.min(amount, room(blocker))
      const back = Math.min(power(blocker), room(attacker))
      if (into > 0) {
        events.push({ DamageDealt: { source: attacker.id, target: { Object: blocker.id }, amount: into } })
        mark(blocker.id, into)
      }
      if (back > 0) {
        events.push({ DamageDealt: { source: blocker.id, target: { Object: attacker.id }, amount: back } })
        mark(attacker.id, back)
      }
    } else if ('Planeswalker' in target) {
      events.push({ DamageDealt: { source: attacker.id, target: { Object: target.Planeswalker }, amount } })
    } else {
      events.push({ DamageDealt: { source: attacker.id, target: { Player: foe }, amount } })
      const player = next.players[foe]
      if (player !== undefined) player.life -= amount
    }
  }
  events.push({ PriorityGranted: { player: next.turn.active } })
  return events
}

/** The strike move of the dev table's effects. */
export const STRIKE_MOVES = { strike }
