/** The words of the dice prompt (`R706ChooseDice`): the roll, each die, and the answer the chosen dice make. */
import type { R706DiceOptions } from '@/protocol/generated'

/** Whether the question is a reroll (any number of dice, none declining) rather than the roll to ignore (exactly one). */
export function isReroll(options: R706DiceOptions): boolean {
  return options.purpose !== 'Ignore'
}

/** The cost of the reroll, "{1}"; empty for the roll to ignore. */
export function rerollCost(options: R706DiceOptions): string {
  return typeof options.purpose === 'object' ? options.purpose.Reroll : ''
}

/** "4, 2 and 6": the results in the order rolled. */
export function resultsWords(results: readonly number[]): string {
  const words = results.map(String)
  if (words.length <= 1) return words.join('')
  return `${words.slice(0, -1).join(', ')} and ${words[words.length - 1]}`
}

/** The roll as a sentence: "You rolled 4 and 2 on six-sided dice." or "Ana rolled a 5 on a d20." */
export function rollWords(roller: string, sides: number, results: readonly number[]): string {
  const die = sides === 6 ? 'six-sided' : `${sides}-sided`
  if (results.length === 1) return `${roller} rolled a ${results[0]} on a ${die} die.`
  return `${roller} rolled ${resultsWords(results)} on ${die} dice.`
}

/** The label of one die: "Die 2: 5". */
export function dieLabel(at: number, result: number): string {
  return `Die ${at + 1}: ${result}`
}

/** The label of the confirming button of a reroll: "Reroll 2 dice ({1})", or the nothing-chosen hint. */
export function rerollLabel(chosen: number, cost: string): string {
  if (chosen === 0) return 'Choose dice to reroll'
  const dice = chosen === 1 ? '1 die' : `${chosen} dice`
  return cost === '' ? `Reroll ${dice}` : `Reroll ${dice} (pay ${cost})`
}
