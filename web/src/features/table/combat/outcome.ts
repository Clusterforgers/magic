/** What a block does to the two creatures in it, read off the engine's forecast (the client never works it out). */
import type { CombatForecast, ObjectId } from '@/protocol/generated'

/** The fate of an attacker and one of its blockers. */
export type Outcome = 'attacker-dies' | 'blocker-dies' | 'both-die' | 'both-survive'

/** Every outcome, in the order the legend lists them. */
export const OUTCOMES: readonly Outcome[] = ['attacker-dies', 'blocker-dies', 'both-die', 'both-survive']

/** Each outcome in words, for the legend and for screen readers. */
export const OUTCOME_WORDS: Record<Outcome, string> = {
  'attacker-dies': 'Attacker dies',
  'blocker-dies': 'Blocker dies',
  'both-die': 'Both die',
  'both-survive': 'Both survive',
}

/** The outcome of a pair from whether each of its creatures dies. */
export function outcomeOf(attackerDies: boolean, blockerDies: boolean): Outcome {
  if (attackerDies) return blockerDies ? 'both-die' : 'attacker-dies'
  return blockerDies ? 'blocker-dies' : 'both-survive'
}

/** The forecast's verdict on one creature, or undefined when the forecast does not list it. */
export function diesIn(forecast: CombatForecast, id: ObjectId): boolean | undefined {
  return forecast.creatures.find((creature) => creature.object === id)?.dies
}

/** The outcome of a block from the forecast, or undefined when it lists either creature no more. */
export function blockOutcome(
  forecast: CombatForecast,
  attacker: ObjectId,
  blocker: ObjectId,
): Outcome | undefined {
  const attackerDies = diesIn(forecast, attacker)
  const blockerDies = diesIn(forecast, blocker)
  return attackerDies === undefined || blockerDies === undefined
    ? undefined
    : outcomeOf(attackerDies, blockerDies)
}
