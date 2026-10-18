/**
 * Dividing an attacker's combat damage among its blockers, as the AssignDamage prompt does it. The engine accepts any
 * split of whole numbers that adds up to the attacker's power (the attacker divides damage freely among blockers);
 * lethal damage is only a hint for now, since it matters to trample, which is not implemented.
 */
import type { Answer, ObjectId } from '@/protocol/generated'

/** One creature that may receive damage. */
export interface DamageRecipient {
  id: ObjectId
  /** What the player calls it; two blockers of one name are told apart by the caller. */
  name: string
  /** Current toughness, or null when the creature has none. */
  toughness: number | null
  /** Damage already marked on it this turn. */
  damage: number
}

/** What a split comes to: whether it can be sent, and why not when it cannot. */
export interface Verdict {
  legal: boolean
  /** The state of the split in plain words, such as why Confirm cannot be pressed. */
  status: string
}

/** What it still takes to destroy the recipient with damage, or null when its toughness is unknown. */
export function lethalDamage({ toughness, damage }: DamageRecipient): number | null {
  return toughness === null ? null : Math.max(0, toughness - damage)
}

/** The sum of the amounts. */
export function totalOf(amounts: readonly number[]): number {
  return amounts.reduce((sum, amount) => sum + amount, 0)
}

/**
 * The split a player most often wants: each recipient in the offered order gets lethal damage while power lasts, and
 * the last one gets whatever remains. It always adds up to `power` unless there is nobody to give it to.
 */
export function defaultSplit(power: number, recipients: readonly DamageRecipient[]): number[] {
  let left = Math.max(0, Math.trunc(power))
  return recipients.map((recipient, index) => {
    const share = index === recipients.length - 1 ? left : Math.min(left, lethalDamage(recipient) ?? 0)
    left -= share
    return share
  })
}

/** Whole numbers from 0 up to the most that fits in `power` next to the other amounts; NaN counts as 0 and infinity as the most. */
export function clampedAmount(
  amounts: readonly number[],
  index: number,
  wanted: number,
  power: number,
): number {
  const others = totalOf(amounts) - (amounts[index] ?? 0)
  const room = Math.max(0, power - others)
  const whole = Number.isNaN(wanted) ? 0 : Math.trunc(wanted)
  return Math.min(Math.max(whole, 0), room)
}

/** `amounts` with the one at `index` changed to `wanted`, kept within what the rules allow; the others stay. */
export function setAmount(
  amounts: readonly number[],
  index: number,
  wanted: number,
  power: number,
): number[] {
  if (index < 0 || index >= amounts.length) return [...amounts]
  return amounts.map((amount, at) => (at === index ? clampedAmount(amounts, index, wanted, power) : amount))
}

/** Whether `amounts` may be sent as the division of `power`, with the reason in words when it may not. */
export function judge(power: number, amounts: readonly number[]): Verdict {
  const assigned = totalOf(amounts)
  if (amounts.length === 0) {
    return power === 0
      ? { legal: true, status: 'There is no damage to assign.' }
      : { legal: false, status: 'There is nothing to assign this damage to.' }
  }
  const sofar = `Assigned ${assigned} of ${power}.`
  if (amounts.some((amount) => !Number.isInteger(amount) || amount < 0)) {
    return { legal: false, status: `${sofar} Every share must be a whole number, 0 or more.` }
  }
  if (assigned < power) {
    return { legal: false, status: `${sofar} Assign ${power - assigned} more to confirm.` }
  }
  if (assigned > power) {
    return { legal: false, status: `${sofar} Take away ${assigned - power} to confirm.` }
  }
  return { legal: true, status: power === 0 ? 'There is no damage to assign.' : `${sofar} Ready to confirm.` }
}

/** The answer for a split: the recipients that get damage, in the offered order. */
export function toAnswer(recipients: readonly DamageRecipient[], amounts: readonly number[]): Answer {
  const shares: [ObjectId, number][] = []
  recipients.forEach((recipient, index) => {
    const amount = amounts[index] ?? 0
    if (amount > 0) shares.push([recipient.id, amount])
  })
  return { AssignDamage: shares }
}

/** How a recipient stands against its lethal damage, in words: "Lethal", "2 short of lethal", or null when unknown. */
export function lethalNote(recipient: DamageRecipient, amount: number): string | null {
  const lethal = lethalDamage(recipient)
  // With lethal damage already marked there is nothing to compare; `factsOf` says so.
  if (lethal === null || lethal === 0) return null
  if (amount >= lethal) return 'Lethal'
  return `${lethal - amount} short of lethal`
}

/** The recipient's numbers in words: toughness, damage already marked and what it takes to destroy it. */
export function factsOf(recipient: DamageRecipient): string {
  const lethal = lethalDamage(recipient)
  if (lethal === null) return 'No toughness'
  const marked = recipient.damage > 0 ? `, ${recipient.damage} damage marked` : ''
  const end = lethal === 0 ? 'lethal damage already marked' : `lethal: ${lethal}`
  return `Toughness ${recipient.toughness}${marked}, ${end}`
}

/** Names for the recipients where two of one name get "(1 of 2)" and "(2 of 2)" so every row can be told apart. */
export function labelled(names: readonly string[]): string[] {
  const counts = new Map<string, number>()
  for (const name of names) counts.set(name, (counts.get(name) ?? 0) + 1)
  const seen = new Map<string, number>()
  return names.map((name) => {
    const total = counts.get(name) ?? 1
    if (total === 1) return name
    const nth = (seen.get(name) ?? 0) + 1
    seen.set(name, nth)
    return `${name} (${nth} of ${total})`
  })
}
