/**
 * Dividing an amount among targets, as the DivideAmount prompt does it: the engine takes every target once, at least
 * `MIN_SHARE` each, and all of the total in sum. The prompt starts from the split the response clock would answer, a
 * number field per target keeps every share inside what the others leave, and Confirm waits until nothing is left.
 */
import type { Answer, Target } from '@/protocol/generated'

/** The least a target gets: the engine refuses a share of nothing. */
export const MIN_SHARE = 1

/** What a split comes to: whether it can be sent, how much is still to give, and the state in plain words. */
export interface Division {
  legal: boolean
  /** The part of the total no target holds yet. */
  left: number
  status: string
}

const sum = (shares: readonly number[]): number => shares.reduce((all, share) => all + share, 0)

/** The total shared as evenly as it goes, the first target taking what is left over, as the server answers at zero. */
export function evenShares(total: number, count: number): number[] {
  if (count <= 0) return []
  const each = Math.max(MIN_SHARE, Math.floor(total / count))
  return Array.from({ length: count }, (_, at) =>
    at === 0 ? Math.max(MIN_SHARE, total - each * (count - 1)) : each,
  )
}

/** The most target `index` may hold: what the total leaves after the other targets, and at least `MIN_SHARE`. */
export function mostFor(shares: readonly number[], index: number, total: number): number {
  return Math.max(MIN_SHARE, total - (sum(shares) - (shares[index] ?? 0)))
}

/** `shares` with target `index` set to `wanted`, a whole number kept from `MIN_SHARE` to what the others leave. */
export function setShare(shares: readonly number[], index: number, wanted: number, total: number): number[] {
  const current = shares[index] ?? MIN_SHARE
  const next = Number.isFinite(wanted) ? Math.round(wanted) : current
  const clamped = Math.min(mostFor(shares, index, total), Math.max(MIN_SHARE, next))
  return shares.map((share, at) => (at === index ? clamped : share))
}

/** Whether `shares` is a split the engine takes, and what is left to give or why there is none. */
export function judgeShares(total: number, shares: readonly number[]): Division {
  const count = shares.length
  const left = total - sum(shares)
  if (count === 0) return { legal: false, left, status: 'There is nothing to divide among.' }
  if (total < count * MIN_SHARE) {
    return { legal: false, left, status: `There is not enough to give each target ${MIN_SHARE}.` }
  }
  if (left > 0) return { legal: false, left, status: `${left} left to give.` }
  if (left < 0) return { legal: false, left, status: `${-left} too many.` }
  return { legal: true, left: 0, status: `All ${total} given out.` }
}

/** The live sentence: the asker's name and the engine's words, else a plain request, and the least each gets. */
export function divideSentence(asker: string, text: string, total: number): string {
  const ask = text === '' ? `divide ${total} among the targets.` : text
  return `${asker}: ${ask} Each target gets at least ${MIN_SHARE}.`
}

/** The answer for `shares`, in the order of `targets`. */
export function divisionAnswer(targets: readonly Target[], shares: readonly number[]): Answer {
  return {
    DivideAmount: {
      Division: targets.map((target, at): [Target, number] => [target, shares[at] ?? MIN_SHARE]),
    },
  }
}
