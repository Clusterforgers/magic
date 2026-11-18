/**
 * The circles of a payment: one slot for each mana a cost asks for, and which piece of the payment so far fills it, so
 * the column can caption every symbol with the source that paid it. For the eye only: what the engine accepts is
 * `payment.ts`'s to say.
 */
import type { ManaCost, ManaSymbol, ManaType, ObjectId } from '@/protocol/generated'
import { waysOf } from '../manaSymbols'
import { costsMana, genericOf, madeOf } from './payment'
import type { LifeSymbols, Spend } from './payment'

/** What fills a slot. */
export interface SlotFill {
  /** The mana that pays it; null for a payment of generic alone (improvise, delve) and for life. */
  mana: ManaType | null
  /** The piece of the payment that made it, by place in the payment; null for life. */
  spend: number | null
  /** Life pays the symbol. */
  life: boolean
}

/** One mana the payment owes. */
export interface PaySlot {
  key: string
  /** The symbol as the cost prints it; a generic part is one `{1}` a slot. */
  symbol: ManaSymbol
  /** The kinds of mana that pay it; null for any kind. */
  accepts: readonly ManaType[] | null
  /** The source whose own cost this is (a Signet's {1}); undefined for the spell's cost. */
  extraOf: ObjectId | undefined
  fill: SlotFill | null
}

const ONE: ManaSymbol = { Generic: 1 }

/** The slots a cost asks for: a generic part one slot a mana, every other symbol one slot. */
function slotsOf(cost: ManaCost, prefix: string, extraOf: ObjectId | undefined, life: LifeSymbols): PaySlot[] {
  const paid = new Set(life)
  return cost.flatMap((symbol, at): PaySlot[] => {
    if (typeof symbol === 'object' && 'Generic' in symbol) {
      return Array.from({ length: symbol.Generic }, (_, nth) => ({
        key: `${prefix}${at}-${nth}`,
        symbol: ONE,
        accepts: null,
        extraOf,
        fill: null,
      }))
    }
    const kinds = waysOf(symbol).flatMap((way) => (way.kind === 'mana' ? [way.mana] : []))
    const any = waysOf(symbol).some((way) => way.kind === 'generic')
    return [
      {
        key: `${prefix}${at}`,
        symbol,
        accepts: any || kinds.length === 0 ? null : kinds,
        extraOf,
        fill: paid.has(at) ? { mana: null, spend: null, life: true } : null,
      },
    ]
  })
}

/**
 * The slots of `cost` and of the own costs of the sources tapped so far, filled by the payment in the order chosen: a
 * mana goes to the open slot that asks for its kind and accepts the fewest others, else to one that takes any.
 */
export function paySlots(cost: ManaCost, spends: readonly Spend[], life: LifeSymbols = []): PaySlot[] {
  const slots = slotsOf(cost, 'c', undefined, life)
  spends.forEach((spend, index) => {
    if (spend.kind === 'tap' && costsMana(spend.option)) {
      slots.push(...slotsOf(spend.option.cost ?? [], `x${index}-`, spend.option.source.object, []))
    }
  })
  const open = (mana: ManaType | null): PaySlot | undefined => {
    const free = slots.filter((slot) => slot.fill === null)
    if (mana !== null) {
      const typed = free
        .filter((slot) => slot.accepts !== null && slot.accepts.includes(mana))
        .sort((a, b) => (a.accepts?.length ?? 0) - (b.accepts?.length ?? 0))
      if (typed[0] !== undefined) return typed[0]
    }
    return free.find((slot) => slot.accepts === null)
  }
  spends.forEach((spend, index) => {
    const units: (ManaType | null)[] = [
      ...madeOf(spend),
      ...Array.from({ length: genericOf(spend) }, () => null),
    ]
    for (const mana of units) {
      const slot = open(mana)
      if (slot !== undefined) slot.fill = { mana, spend: index, life: false }
    }
  })
  return slots
}

/** How many of the slots are filled. */
export function paidCount(slots: readonly PaySlot[]): number {
  return slots.filter((slot) => slot.fill !== null).length
}
