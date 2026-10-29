/** The ChooseNumber prompt's arithmetic and words: which numbers are allowed, the step to the next one, the total cost for an X. */
import type { ManaCost, NumberOptions } from '@/protocol/generated'

/** The most a number with no `max` may be stepped or typed up to. */
export const OPEN_MAX = 99

/** The largest number the prompt offers: `max`, or `OPEN_MAX` when the decision sets no limit. */
export function topOf(options: NumberOptions): number {
  return options.max ?? Math.max(OPEN_MAX, options.min)
}

/** True when `n` may be answered: a whole number from `min` to `max` that `except` does not name. */
export function isAllowed(options: NumberOptions, n: number): boolean {
  return (
    Number.isInteger(n) && n >= options.min && n <= topOf(options) && !(options.except ?? []).includes(n)
  )
}

/** The allowed number nearest to `from` in the direction `by` (+1 or -1), or `from` when there is none that way. */
export function step(options: NumberOptions, from: number, by: 1 | -1): number {
  for (let n = from + by; n >= options.min && n <= topOf(options); n += by) {
    if (isAllowed(options, n)) return n
  }
  return from
}

/** The number the prompt starts on: the smallest allowed one, the answer the response clock gives. */
export function firstAllowed(options: NumberOptions): number {
  return isAllowed(options, options.min) ? options.min : step(options, options.min, 1)
}

/** The total mana cost for X = `n`: the cost at X = 0 with `per_x` generic mana for each 1 of X. */
export function totalFor(cost: ManaCost, perX: number, n: number): ManaCost {
  const more = perX * n
  if (more === 0) return cost
  const [first, ...rest] = cost
  if (first !== undefined && typeof first === 'object' && 'Generic' in first) {
    return [{ Generic: first.Generic + more }, ...rest]
  }
  return [{ Generic: more }, ...cost]
}

/** The sentence of the prompt: the asker and the engine's words, or a plain question. */
export function numberSentence(asker: string, text: string): string {
  return `${asker}: ${text === '' ? 'choose a number.' : text}`
}

/** Why Confirm is off for `n`, or `undefined` when `n` may be answered. */
export function numberProblem(options: NumberOptions, n: number): string | undefined {
  if (!Number.isInteger(n)) return 'Enter a whole number.'
  if (n < options.min) return `Choose ${options.min} or more.`
  if (n > topOf(options)) return `Choose ${topOf(options)} or less.`
  if ((options.except ?? []).includes(n)) return `${n} is not allowed here.`
  return undefined
}

/** The range for a screen reader: "From 0 to 4, except 2." */
export function rangeWords(options: NumberOptions): string {
  const except = options.except ?? []
  const range = options.max === null ? `${options.min} or more` : `From ${options.min} to ${options.max}`
  return except.length === 0 ? `${range}.` : `${range}, except ${except.join(', ')}.`
}
