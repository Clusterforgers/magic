/**
 * The trigger-order prompt: the list on screen is in resolution order (the top trigger resolves first), while the
 * answer lists the triggers in the order they go on the stack, the last on top. These are the two orders and the words.
 */
import type { TriggerEntry } from '@/protocol/generated'
import { countOf } from './combatText'

/** The order the engine takes without a choice, as the list shows it: the trigger that triggered last resolves first. */
export function defaultOrder(count: number): number[] {
  return Array.from({ length: count }, (_, index) => count - 1 - index)
}

/** The answer for a list: the same triggers in the order they go on the stack, so the one on top of the list is last. */
export function answerOf(shown: readonly number[]): number[] {
  return [...shown].reverse()
}

/** The list with the trigger at `place` moved one place up (-1) or down (1); unchanged at either end. */
export function moved(shown: readonly number[], place: number, by: -1 | 1): number[] {
  const next = [...shown]
  const to = place + by
  const [one, other] = [next[place], next[to]]
  if (one === undefined || other === undefined) return next
  next[place] = other
  next[to] = one
  return next
}

/** Whether `shown` is the default order of its triggers. */
export function isDefaultOrder(shown: readonly number[]): boolean {
  const normal = defaultOrder(shown.length)
  return shown.every((entry, place) => entry === normal[place])
}

/**
 * How each trigger is told apart in the list: its source, then what it waits for in the words of a card ("When it
 * enters the battlefield"), or its ability number when the engine gave no words. Triggers that still read the same
 * are numbered in the order they triggered, so a label stays with its trigger when the list is rearranged.
 */
export function triggerLabels(
  entries: readonly TriggerEntry[],
  names: readonly (string | undefined)[],
): string[] {
  const plain = entries.map((entry, index) => {
    const name = names[index] ?? 'A card'
    const when = entry.when ?? ''
    return `${name}: ${when === '' ? `ability ${entry.ability + 1}` : when}`
  })
  const total = new Map<string, number>()
  for (const label of plain) total.set(label, (total.get(label) ?? 0) + 1)
  const seen = new Map<string, number>()
  return plain.map((label) => {
    if ((total.get(label) ?? 0) < 2) return label
    const nth = (seen.get(label) ?? 0) + 1
    seen.set(label, nth)
    return `${label} (${nth})`
  })
}

/** "1st", "2nd", "3rd", "4th", "11th", "21st". */
export function ordinal(n: number): string {
  const tens = n % 100
  if (tens >= 11 && tens <= 13) return `${n}th`
  return `${n}${['th', 'st', 'nd', 'rd'][n % 10] ?? 'th'}`
}

/** Where a trigger stands in the list, for the sentence that follows a move: "now 2nd of 3", "now 1st of 3, resolves first". */
export function placeWords(place: number, count: number): string {
  const tail = place === 0 ? ', resolves first' : place === count - 1 ? ', resolves last' : ''
  return `now ${ordinal(place + 1)} of ${count}${tail}`
}

/** The sentence above the list. */
export function orderSentence(count: number, note: string | null): string {
  const head = `Order your ${countOf(count, 'trigger')}: the top one resolves first.`
  return note === null ? `${head} Confirm when the order is right.` : `${head} ${note}`
}
