/**
 * The arithmetic of paying a mana cost by hand, the way the engine checks it: the payment must cover every coloured
 * and colourless requirement and have exactly the cost's total, so nothing may be added that would be wasted.
 */
import type { ManaCost, ManaOption, ManaPool, ManaType, Payment } from '@/protocol/generated'

/** The kinds of mana in the order of `ManaPool.counts`; `ManaCost.colored` is the first five. */
export const MANA_TYPES: readonly ManaType[] = ['White', 'Blue', 'Black', 'Red', 'Green', 'Colorless']

/** One piece of the running payment: floating mana, or a permanent's mana ability. */
export type Spend = { kind: 'pool'; mana: ManaType } | { kind: 'tap'; option: ManaOption }

/** The mana a piece of the payment makes. */
export function manaOf(spend: Spend): ManaType {
  return spend.kind === 'pool' ? spend.mana : spend.option.produces
}

/** How many of each kind of mana the payment holds, in `MANA_TYPES` order. */
export function tally(spends: readonly Spend[]): number[] {
  return MANA_TYPES.map((type) => spends.filter((spend) => manaOf(spend) === type).length)
}

/** What the cost asks for in each kind of mana (generic not included), in `MANA_TYPES` order. */
function required(cost: ManaCost): number[] {
  return [...cost.colored, cost.colorless]
}

/** The cost's total: generic, colourless and coloured symbols together. */
export function manaValue(cost: ManaCost): number {
  return cost.generic + cost.colorless + cost.colored.reduce((sum, count) => sum + count, 0)
}

/** Mana held beyond what each kind is required to be; it can only go toward the generic part. */
function surplus(cost: ManaCost, have: readonly number[]): number {
  return required(cost).reduce((sum, need, index) => sum + Math.max(0, (have[index] ?? 0) - need), 0)
}

/** What is still owed after `spends`: the cost with everything the payment already covers taken off. */
export function owed(cost: ManaCost, spends: readonly Spend[]): ManaCost {
  const have = tally(spends)
  const short = (index: number, need: number): number => Math.max(0, need - (have[index] ?? 0))
  return {
    generic: Math.max(0, cost.generic - surplus(cost, have)),
    colorless: short(5, cost.colorless),
    colored: [
      short(0, cost.colored[0]),
      short(1, cost.colored[1]),
      short(2, cost.colored[2]),
      short(3, cost.colored[3]),
      short(4, cost.colored[4]),
    ],
  }
}

/** True when `spends` is a payment the engine accepts: nothing owed and exactly the cost's total. */
export function covers(cost: ManaCost, spends: readonly Spend[]): boolean {
  return spends.length === manaValue(cost) && manaValue(owed(cost, spends)) === 0
}

/**
 * True when one more `mana` would still go toward the cost. Mana beyond the coloured requirements goes to the
 * generic part, so it fits only while generic mana is owed; anything else is a payment the engine would refuse.
 */
export function fits(cost: ManaCost, spends: readonly Spend[], mana: ManaType): boolean {
  const have = tally(spends)
  const index = MANA_TYPES.indexOf(mana)
  have[index] = (have[index] ?? 0) + 1
  return surplus(cost, have) <= cost.generic
}

/** The payment as the wire takes it: the floating mana to spend and the abilities to tap. */
export function toPayment(spends: readonly Spend[]): Payment {
  return {
    pool: spends.flatMap((spend) => (spend.kind === 'pool' ? [spend.mana] : [])),
    taps: spends.flatMap((spend) => (spend.kind === 'tap' ? [spend.option.source] : [])),
  }
}

/** How much of the pool `spends` has used, of one kind of mana. */
export function poolUsed(spends: readonly Spend[], mana: ManaType): number {
  return spends.filter((spend) => spend.kind === 'pool' && spend.mana === mana).length
}

/** How much of one kind of mana the pool still holds after `spends`. */
export function poolLeft(pool: ManaPool, spends: readonly Spend[], mana: ManaType): number {
  return (pool.counts[MANA_TYPES.indexOf(mana)] ?? 0) - poolUsed(spends, mana)
}

/**
 * True when `spend` may join the payment: it would not be wasted, floating mana is left for it, and a permanent is
 * tapped once (the engine would tap it again for a second ability).
 */
export function allowed(cost: ManaCost, pool: ManaPool, spends: readonly Spend[], spend: Spend): boolean {
  if (!fits(cost, spends, manaOf(spend))) return false
  if (spend.kind === 'pool') return poolLeft(pool, spends, spend.mana) > 0
  return !spends.some(
    (other) => other.kind === 'tap' && other.option.source.object === spend.option.source.object,
  )
}

/** The options of one permanent that may join the payment, one per kind of mana (two abilities of one kind are alike). */
export function fittingOptions(
  cost: ManaCost,
  pool: ManaPool,
  spends: readonly Spend[],
  abilities: readonly ManaOption[],
): ManaOption[] {
  const seen = new Set<ManaType>()
  return abilities.filter((option) => {
    if (seen.has(option.produces) || !allowed(cost, pool, spends, { kind: 'tap', option })) return false
    seen.add(option.produces)
    return true
  })
}
