/** The colour switch of a chosen source: another ability of the same permanent takes the place of the one chosen. */
import type { ManaCost, ManaOption, ManaPool, ObjectId } from '@/protocol/generated'
import { canFund, completable, optionKey, usableByHand } from './payment'
import type { LifeSymbols, Reach, Spend } from './payment'

/** The payment with the tap at `index` using `option` instead; every other piece keeps its place. */
export function withSwap(spends: readonly Spend[], index: number, option: ManaOption): Spend[] {
  return spends.map((spend, at) => (at === index && spend.kind === 'tap' ? { kind: 'tap', option } : spend))
}

/** The place of the last tap of `object` in the payment, or -1. */
export function lastTapOf(spends: readonly Spend[], object: ObjectId): number {
  return spends.findLastIndex((spend) => spend.kind === 'tap' && spend.option.source.object === object)
}

/**
 * The other abilities of the permanent tapped at `index` that could take its place: one per kind of use, usable by
 * hand, every source that costs mana still paid for by the mana before it, and the payment still finishable.
 */
export function swapOptions(
  cost: ManaCost,
  pool: ManaPool,
  spends: readonly Spend[],
  index: number,
  abilities: readonly ManaOption[],
  life: LifeSymbols,
  reach: Reach,
): ManaOption[] {
  const chosen = spends[index]
  if (chosen?.kind !== 'tap') return []
  const seen = new Set([optionKey(chosen.option)])
  return abilities.filter((option) => {
    const key = optionKey(option)
    if (seen.has(key)) return false
    seen.add(key)
    if (!usableByHand(option, reach.spend)) return false
    const next = withSwap(spends, index, option)
    const funded = next.every((spend, at) => spend.kind !== 'tap' || canFund(next.slice(0, at), spend.option))
    return funded && completable(cost, pool, next, life, reach)
  })
}
