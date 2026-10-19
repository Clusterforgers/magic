/**
 * Dividing an attacker's combat damage among its blockers, as the AssignDamage prompt does it. Without trample the
 * engine takes any split of whole numbers that adds up to the attacker's power. With trample the split may fall short:
 * what is left goes to the player or planeswalker attacked, which the engine lets take damage only once every blocker
 * has at least its lethal damage. The engine states the lethal damage of each blocker (deathtouch and damage already
 * marked counted); the hint works it out from the toughness and the attacker's keywords only for options without it.
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
  /** The damage the engine says is lethal to it, when the decision carries it. */
  lethal?: number | undefined
}

/** How the attacker's damage may go past its blockers, when it has trample. */
export interface Trample {
  /** Who takes what is not given to a blocker, by name. */
  to: string
  /** The blockers, in the order of the amounts. */
  recipients: readonly DamageRecipient[]
  /** Whether the attacker has deathtouch, which makes 1 damage lethal. */
  deathtouch?: boolean
}

/** What a split comes to: whether it can be sent, and why not when it cannot. */
export interface Verdict {
  legal: boolean
  /** The state of the split in plain words, such as why Confirm cannot be pressed. */
  status: string
}

/** Whether the keyword names of an attacker include deathtouch. */
export function hasDeathtouch(keywords: readonly string[] | undefined): boolean {
  return keywords?.includes('Deathtouch') ?? false
}

/** What the toughness leaves after the damage marked, or null when the creature has no toughness. */
function remaining({ toughness, damage }: DamageRecipient): number | null {
  return toughness === null ? null : Math.max(0, toughness - damage)
}

/**
 * What it still takes to destroy the recipient with damage from this attacker, or null when its toughness is unknown.
 *
 * The engine's figure wins. Without one it is what the toughness leaves, and 1 at most from an attacker with
 * deathtouch, since any damage from such a source is lethal.
 */
export function lethalDamage(recipient: DamageRecipient, deathtouch = false): number | null {
  if (recipient.lethal !== undefined) return recipient.lethal
  const left = remaining(recipient)
  return left !== null && deathtouch ? Math.min(left, 1) : left
}

/** The sum of the amounts. */
export function totalOf(amounts: readonly number[]): number {
  return amounts.reduce((sum, amount) => sum + amount, 0)
}

/** How a split is made: whether the attacker has trample and whether deathtouch shortens what is lethal. */
export interface SplitRule {
  trample?: boolean
  deathtouch?: boolean
}

/**
 * The split a player most often wants: each recipient in the offered order gets lethal damage while power lasts, and
 * the last one gets whatever remains. With trample what remains is left for the player or planeswalker attacked
 * instead. Without trample it adds up to `power` unless there is nobody to give it to.
 */
export function defaultSplit(
  power: number,
  recipients: readonly DamageRecipient[],
  { trample = false, deathtouch = false }: SplitRule = {},
): number[] {
  let left = Math.max(0, Math.trunc(power))
  return recipients.map((recipient, index) => {
    const last = index === recipients.length - 1 && !trample
    const share = last ? left : Math.min(left, lethalDamage(recipient, deathtouch) ?? 0)
    left -= share
    return share
  })
}

/** What is left of the power once the amounts are given: with trample, the share of the player or planeswalker attacked. */
export function restOf(power: number, amounts: readonly number[]): number {
  return Math.max(0, Math.trunc(power) - totalOf(amounts))
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

/** Whether every recipient has at least its lethal damage: the condition for anything to go past it with trample. */
export function lethalEverywhere(
  recipients: readonly DamageRecipient[],
  amounts: readonly number[],
  deathtouch = false,
): boolean {
  return recipients.every(
    (recipient, index) => (amounts[index] ?? 0) >= (lethalDamage(recipient, deathtouch) ?? 0),
  )
}

/**
 * Whether `amounts` may be sent as the division of `power`, with the reason in words when it may not.
 *
 * Without `trample` all the power goes to the recipients. With it a split may fall short, and the rest goes to
 * `trample.to` if every recipient has at least its lethal damage.
 */
export function judge(power: number, amounts: readonly number[], trample?: Trample): Verdict {
  const assigned = totalOf(amounts)
  if (amounts.length === 0) {
    if (trample !== undefined && power > 0)
      return { legal: true, status: `All ${power} goes to ${trample.to}.` }
    return power === 0
      ? { legal: true, status: 'There is no damage to assign.' }
      : { legal: false, status: 'There is nothing to assign this damage to.' }
  }
  const sofar = `Assigned ${assigned} of ${power}.`
  if (amounts.some((amount) => !Number.isInteger(amount) || amount < 0)) {
    return { legal: false, status: `${sofar} Every share must be a whole number, 0 or more.` }
  }
  if (assigned > power) {
    return { legal: false, status: `${sofar} Take away ${assigned - power} to confirm.` }
  }
  if (assigned < power) {
    if (trample === undefined) {
      return { legal: false, status: `${sofar} Assign ${power - assigned} more to confirm.` }
    }
    if (!lethalEverywhere(trample.recipients, amounts, trample.deathtouch)) {
      return {
        legal: false,
        status: `${sofar} Give every blocker its lethal damage to send the rest to ${trample.to}, or assign ${power - assigned} more.`,
      }
    }
    return {
      legal: true,
      status: `${sofar} The other ${power - assigned} goes to ${trample.to}. Ready to confirm.`,
    }
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

/** How a recipient stands against its lethal damage: "Lethal" or "2 short of lethal", null when unknown or already lethal. */
export function lethalNote(recipient: DamageRecipient, amount: number, deathtouch = false): string | null {
  const lethal = lethalDamage(recipient, deathtouch)
  // With lethal damage already marked there is nothing to compare; `factsOf` says so.
  if (lethal === null || lethal === 0) return null
  if (amount >= lethal) return 'Lethal'
  return `${lethal - amount} short of lethal`
}

/** The recipient's numbers in words: toughness, damage already marked and what it takes to destroy it. */
export function factsOf(recipient: DamageRecipient, deathtouch = false): string {
  const lethal = lethalDamage(recipient, deathtouch)
  if (lethal === null) return 'No toughness'
  const marked = recipient.damage > 0 ? `, ${recipient.damage} damage marked` : ''
  const left = remaining(recipient) ?? 0
  const because = deathtouch && lethal < left ? ' (deathtouch)' : ''
  const end = lethal === 0 ? 'lethal damage already marked' : `lethal: ${lethal}${because}`
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
