/**
 * Combat scenes for the dev table that carry a forecast: `forecastScene` (`?prompt=forecast`) shows every outcome the
 * forecast can have at once, and `withBlockForecast` gives the declare-blockers fixture (`?prompt=block`) the
 * forecast the engine would send before the viewer has blocked.
 */
import type { AttackerView, ObjectId, ObjectView, PlayerId, StateView } from '@/protocol/generated'
import { devForecast } from './devForecast'
import type { Fixture } from './fixture'

/** One attacker's stats with the stats of the creatures that block it (none for an unblocked attacker). */
interface Duel {
  attacker: readonly [power: number, toughness: number]
  blockers: readonly (readonly [power: number, toughness: number])[]
}

/**
 * Stats chosen so that the forecast has one of everything: a double block where the attacker and the first blocker
 * die and the second blocker lives, a block where only the blocker dies, one where both live, one where only the
 * attacker dies, and an unblocked attacker that is lethal to a defender on 5 life.
 */
const DUELS: readonly Duel[] = [
  {
    attacker: [4, 4],
    blockers: [
      [3, 3],
      [2, 2],
    ],
  },
  { attacker: [5, 3], blockers: [[1, 2]] },
  { attacker: [2, 4], blockers: [[1, 4]] },
  { attacker: [1, 1], blockers: [[3, 3]] },
  { attacker: [6, 6], blockers: [] },
]

/** The life the defender is on, so that the unblocked attacker's 6 damage is lethal. */
const DEFENDER_LIFE = 5

/** Creatures of `seat` that can fight and carry no damage, in battlefield order. */
const fighters = (view: StateView, seat: PlayerId): ObjectView[] =>
  view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' &&
      object.controller === seat &&
      object.types.includes('Creature') &&
      object.permanent?.summoning_sick === false &&
      object.permanent.damage === 0,
  )

const withLife = (view: StateView, seat: PlayerId, life: number): StateView => ({
  ...view,
  players: view.players.map((player) => (player.seat === seat ? { ...player, life } : player)),
})

/**
 * A declare-blockers moment in which the active seat has attacked the defender with five creatures and the defender
 * has blocked as `DUELS` says, with the forecast for it. The defender is the viewer, or the next seat when the viewer
 * is the one attacking, and is left on 5 life.
 */
export function forecastScene(fixture: Fixture): Fixture {
  const { view } = fixture
  const active = view.turn.active
  const defender = view.viewer === active ? (view.viewer + 1) % view.players.length : view.viewer
  const attackers = fighters(view, active)
  // Blockers must be untapped; the creatures tapped by attacking are never among them.
  const blockers = fighters(view, defender).filter((object) => object.permanent?.tapped === false)

  const stats = new Map<ObjectId, readonly [number, number]>()
  const seated: AttackerView[] = []
  let nextBlocker = 0
  for (const [index, duel] of DUELS.entries()) {
    const attacker = attackers[index]
    const against = blockers.slice(nextBlocker, nextBlocker + duel.blockers.length)
    if (attacker === undefined || against.length < duel.blockers.length) break
    nextBlocker += duel.blockers.length
    stats.set(attacker.id, duel.attacker)
    against.forEach((blocker, at) => stats.set(blocker.id, duel.blockers[at] ?? [1, 1]))
    seated.push({
      object: attacker.id,
      target: { Player: defender },
      blocked: against.length > 0,
      blockers: against.map((blocker) => blocker.id),
    })
  }
  const attacking = new Set(seated.map((attacker) => attacker.object))
  const objects = view.objects.map((object): ObjectView => {
    const pair = stats.get(object.id)
    const stated = pair === undefined ? object : { ...object, power: pair[0], toughness: pair[1] }
    return attacking.has(object.id) && stated.permanent !== null
      ? { ...stated, permanent: { ...stated.permanent, tapped: true } }
      : stated
  })
  const staged: StateView = withLife(
    {
      ...view,
      objects,
      turn: { ...view.turn, step: 'DeclareBlockers' },
      combat: { attackers: seated, attackers_declared: true, blockers_declared: [defender] },
    },
    defender,
    DEFENDER_LIFE,
  )
  return { ...fixture, view: { ...staged, combat_forecast: devForecast(staged) } }
}

/**
 * The declare-blockers fixture with the forecast the engine would send now: nothing is blocked by the viewer yet, so
 * the attackers aimed at the viewer are unblocked and, when there is a third seat, its player has double-blocked one
 * of the attackers aimed at them. The viewer is left on one life less than those attackers deal, so it is lethal.
 */
export function withBlockForecast(fixture: Fixture): Fixture {
  const { view } = fixture
  // The first attacker aimed at someone other than the viewer, and that player's first two untapped creatures.
  const victim = view.combat.attackers.find(
    (attacker) => 'Player' in attacker.target && attacker.target.Player !== view.viewer,
  )
  const other = victim !== undefined && 'Player' in victim.target ? victim.target.Player : undefined
  const walls =
    other === undefined
      ? []
      : fighters(view, other)
          .filter((object) => object.permanent?.tapped === false)
          .slice(0, 2)
          .map((object) => object.id)
  const doubled = other !== undefined && walls.length === 2
  const attackers = view.combat.attackers.map((attacker) =>
    doubled && attacker.object === victim?.object
      ? { ...attacker, blocked: true, blockers: walls }
      : attacker,
  )
  const staged: StateView = {
    ...view,
    combat: { ...view.combat, attackers, blockers_declared: doubled ? [other] : [] },
  }
  const damage = devForecast(staged).players.find((player) => player.player === view.viewer)?.damage ?? 0
  const lethal = withLife(staged, view.viewer, Math.max(1, damage - 1))
  return { ...fixture, view: { ...lethal, combat_forecast: devForecast(lethal) } }
}
