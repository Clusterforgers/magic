/**
 * A combat forecast for the dev table, worked out from the cards of a fixture. Dev only: the real forecast comes from
 * the engine (`crates/mtg-engine/src/forecast.rs`), and the table never works one out. This repeats the engine's
 * plain rules (damage equals power, lethal damage to each blocker in turn and the rest on the last, a creature dies
 * when marked plus new damage reaches its toughness) so that a fixture's numbers agree with the cards it shows.
 */
import type {
  AttackerForecast,
  CombatForecast,
  CreatureForecast,
  ForecastDamage,
  ObjectId,
  PlayerForecast,
  PlayerId,
  StateView,
} from '@/protocol/generated'

/** Lethal damage to each in turn, then everything left over to the last one. */
function divide(power: number, lethal: readonly number[]): number[] {
  let left = power
  return lethal.map((needed, index) => {
    const share = index === lethal.length - 1 ? left : Math.min(left, needed)
    left -= share
    return share
  })
}

/** What combat damage would do to the combat in `view`, as the engine would say it. */
export function devForecast(view: StateView): CombatForecast {
  const byId = new Map(view.objects.map((object) => [object.id, object]))
  const power = (id: ObjectId): number => Math.max(0, byId.get(id)?.power ?? 0)
  const marked = (id: ObjectId): number => byId.get(id)?.permanent?.damage ?? 0
  const lethal = (id: ObjectId): number => Math.max(0, (byId.get(id)?.toughness ?? 0) - marked(id))

  const toObjects = new Map<ObjectId, number>()
  const toPlayers = new Map<PlayerId, number>()
  const involved = new Set<ObjectId>()
  const add = <K>(totals: Map<K, number>, key: K, amount: number): void => {
    totals.set(key, (totals.get(key) ?? 0) + amount)
  }
  const attackers = view.combat.attackers.map((attacker): AttackerForecast => {
    const source = attacker.object
    const strike = power(source)
    const deals: ForecastDamage[] = []
    if (!attacker.blocked) {
      deals.push({
        source,
        target:
          'Player' in attacker.target
            ? { Player: attacker.target.Player }
            : { Object: attacker.target.Planeswalker },
        amount: strike,
      })
    } else {
      const shares = divide(strike, attacker.blockers.map(lethal))
      attacker.blockers.forEach((blocker, index) => {
        deals.push({ source, target: { Object: blocker }, amount: shares[index] ?? 0 })
      })
    }
    const receives = attacker.blockers.map((blocker): ForecastDamage => ({
      source: blocker,
      target: { Object: source },
      amount: power(blocker),
    }))
    involved.add(source)
    for (const blocker of attacker.blockers) involved.add(blocker)
    for (const hit of [...deals, ...receives]) {
      if ('Player' in hit.target) add(toPlayers, hit.target.Player, hit.amount)
      else add(toObjects, hit.target.Object, hit.amount)
    }
    return {
      attacker: source,
      target: attacker.target,
      blocked: attacker.blocked,
      blockers: attacker.blockers,
      deals,
      receives,
    }
  })
  for (const id of toObjects.keys()) involved.add(id)

  const creatures = [...involved]
    .sort((a, b) => a - b)
    .flatMap((id): CreatureForecast[] => {
      const object = byId.get(id)
      if (object === undefined || !object.types.includes('Creature')) return []
      const damage = toObjects.get(id) ?? 0
      const toughness = object.toughness ?? 0
      return [{ object: id, damage, dies: toughness <= 0 || marked(id) + damage >= toughness }]
    })
  const attacked = new Set<PlayerId>(toPlayers.keys())
  for (const attacker of view.combat.attackers)
    if ('Player' in attacker.target) attacked.add(attacker.target.Player)
  const players = [...attacked]
    .sort((a, b) => a - b)
    .flatMap((seat): PlayerForecast[] => {
      const player = view.players.find((candidate) => candidate.seat === seat)
      if (player === undefined) return []
      const damage = toPlayers.get(seat) ?? 0
      const after = player.life - damage
      return [
        { player: seat, damage, life: player.life, life_after: after, lethal: damage > 0 && after <= 0 },
      ]
    })
  return { attackers, creatures, players }
}
