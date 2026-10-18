/** The combat forecast in plain words, for the prompt, for screen readers and for the title of a card. Pure. */
import type { AttackTarget, CombatForecast, ObjectId, PlayerForecast, PlayerId } from '@/protocol/generated'
import type { CreatureOutlook } from './forecast'

/** How the forecast's creatures and players are called in a sentence. */
export interface ForecastNames {
  /** "Bo's Hill Giant", "your Grizzly Bears" (capitalised where a sentence starts with it). */
  creature(id: ObjectId): string
  /** "Alice", or "you" for the viewer. */
  player(seat: PlayerId): string
  /** What an attacker attacks: a player's name or the planeswalker as a creature is named. */
  target(target: AttackTarget): string
}

const capital = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1)

/** "a", "a and b", "a, b and c". */
export function listOf(items: readonly string[]): string {
  const last = items.at(-1)
  if (last === undefined) return ''
  return items.length === 1 ? last : `${items.slice(0, -1).join(', ')} and ${last}`
}

/** "1 damage", "4 damage". */
const damageOf = (amount: number): string => `${amount} damage`

/**
 * What the forecast says, one sentence per attacker and per attacked player: who is blocked by whom, who would die,
 * how much an unblocked attacker would deal and how each attacked player's life would change.
 */
export function forecastSentences(forecast: CombatForecast, names: ForecastNames): string[] {
  const dies = new Map(forecast.creatures.map((creature) => [creature.object, creature.dies]))
  const sentences: string[] = []
  for (const attack of forecast.attackers) {
    // A sentence starts with a capital, so "your Elf" becomes "Your Elf" there and stays lower case elsewhere.
    const attacker = capital(names.creature(attack.attacker))
    if (!attack.blocked) {
      const dealt = attack.deals.at(0)?.amount ?? 0
      const hit = dealt === 0 ? 'no damage' : damageOf(dealt)
      sentences.push(`${attacker} is unblocked and would deal ${hit} to ${names.target(attack.target)}.`)
      continue
    }
    if (attack.blockers.length === 0) {
      sentences.push(`${attacker} was blocked but nothing blocks it now, so it would deal no damage.`)
      continue
    }
    const fates = [attack.attacker, ...attack.blockers].flatMap((id) => {
      const fate = dies.get(id)
      return fate === undefined ? [] : [`${names.creature(id)} would ${fate ? 'die' : 'survive'}`]
    })
    const blockers = listOf(attack.blockers.map((id) => names.creature(id)))
    sentences.push(
      `${attacker} is blocked by ${blockers}${fates.length === 0 ? '' : `: ${fates.join(', ')}`}.`,
    )
  }
  for (const player of forecast.players) {
    const who = capital(names.player(player.player))
    sentences.push(
      player.damage === 0
        ? `${who} would take no damage.`
        : `${who} would take ${damageOf(player.damage)}, from ${player.life} to ${player.life_after} life${player.lethal ? ', which is lethal' : ''}.`,
    )
  }
  return sentences
}

/** What a card's spoken name gains from its outlook: ", would take 3 damage and die". Nothing without one. */
export function outlookWords(outlook: CreatureOutlook | null): string {
  if (outlook === null) return ''
  if (outlook.damage === 0) return ', would die'
  return `, would take ${damageOf(outlook.damage)}${outlook.dies ? ' and die' : ''}`
}

/** A seat's life under the forecast in words, for its panel: "Life would go from 20 to 16, which is lethal". */
export function lifeWords(player: PlayerForecast): string {
  if (player.damage === 0) return 'Takes no combat damage'
  return `Life would go from ${player.life} to ${player.life_after}${player.lethal ? ', which is lethal' : ''}`
}
