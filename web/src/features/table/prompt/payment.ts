/**
 * The arithmetic of paying a mana cost by hand, the way the engine checks it. A cost with hybrid or Phyrexian
 * symbols is first resolved (CR 601.2b): each such symbol is paid in one of its ways, which leaves some mana of
 * given kinds, some generic mana and some life. The payment is one the engine accepts when some resolution takes
 * exactly the life paid and exactly the mana chosen: every unit of a kind met, the rest generic, nothing left over.
 * A plain cost has one resolution, which is the cost itself.
 */
import type { ManaCost, ManaOption, ManaPool, ManaSymbol, ManaType, Payment } from '@/protocol/generated'
import { canPayLife, costValue, isPlain, LIFE_PER_SYMBOL, symbolText, waysOf } from '../manaSymbols'

/** The kinds of mana in the order of `ManaPool.counts`. */
export const MANA_TYPES: readonly ManaType[] = ['White', 'Blue', 'Black', 'Red', 'Green', 'Colorless']

/** One piece of the running payment: floating mana, or a permanent's mana ability. */
export type Spend = { kind: 'pool'; mana: ManaType } | { kind: 'tap'; option: ManaOption }

/** The symbols of a cost (by place in it) that the payment pays with life instead of mana. */
export type LifeSymbols = readonly number[]

/** The mana a piece of the payment makes. */
export function manaOf(spend: Spend): ManaType {
  return spend.kind === 'pool' ? spend.mana : spend.option.produces
}

/** How many of each kind of mana the payment holds, in `MANA_TYPES` order. */
export function tally(spends: readonly Spend[]): number[] {
  return MANA_TYPES.map((type) => spends.filter((spend) => manaOf(spend) === type).length)
}

/** The cost's mana value (CR 202.3): `{2/W}` is 2, every other symbol its own value. */
export function manaValue(cost: ManaCost): number {
  return costValue(cost)
}

/** One unit the cost asks for once its choices are made: mana of a kind, or any mana (`null`), from the symbol at `origin`. */
export interface Unit {
  need: ManaType | null
  origin: number
}

/** Every way to split `total` copies of a symbol among `ways`, the first way taking the most first. */
function splits(total: number, ways: number): number[][] {
  if (ways <= 1) return [[total]]
  const found: number[][] = []
  for (let take = total; take >= 0; take -= 1) {
    for (const rest of splits(total - take, ways - 1)) found.push([take, ...rest])
  }
  return found
}

/**
 * Every way to make the cost's choices when `life` names the symbols that life pays: the units each leaves to pay
 * with mana. Equal symbols sit side by side in a cost, so their copies are split among the ways, not permuted.
 */
export function resolutions(cost: ManaCost, life: LifeSymbols = []): Unit[][] {
  const paid = new Set(life)
  const groups: { symbol: ManaSymbol; origins: number[] }[] = []
  cost.forEach((symbol, index) => {
    if (paid.has(index)) return
    const last = groups.at(-1)
    if (last !== undefined && symbolText(last.symbol) === symbolText(symbol)) last.origins.push(index)
    else groups.push({ symbol, origins: [index] })
  })
  let found: Unit[][] = [[]]
  for (const { symbol, origins } of groups) {
    const ways = waysOf(symbol).filter((way) => way.kind !== 'life')
    const options = splits(origins.length, ways.length)
    found = found.flatMap((units) =>
      options.map((split) => {
        const next = [...units]
        let copy = 0
        ways.forEach((way, at) => {
          for (let n = 0; n < (split[at] ?? 0); n += 1) {
            const origin = origins[copy]
            copy += 1
            if (origin === undefined) continue
            if (way.kind === 'mana') next.push({ need: way.mana, origin })
            if (way.kind === 'generic') {
              for (let g = 0; g < way.count; g += 1) next.push({ need: null, origin })
            }
          }
        })
        return next
      }),
    )
  }
  return found
}

/** What the units still ask for after the payment: the kinds not met, the generic part not met, and mana with no place. */
interface Placement {
  owed: Unit[]
  genericOwed: number
  waste: number
}

/** Puts the payment's mana against the units: each kind meets its own, and what is left over goes to generic. */
function place(cost: ManaCost, units: readonly Unit[], spends: readonly Spend[]): Placement {
  const have = tally(spends)
  const owed: Unit[] = []
  let leftover = 0
  // A plain symbol meets mana first, so a hybrid one stays open as long as it can.
  const plainFirst = (unit: Unit): number => {
    const symbol = cost[unit.origin]
    return symbol !== undefined && isPlain(symbol) ? 0 : 1
  }
  MANA_TYPES.forEach((type, index) => {
    const typed = units
      .filter((unit) => unit.need === type)
      .sort((a, b) => plainFirst(a) - plainFirst(b) || a.origin - b.origin)
    const got = have[index] ?? 0
    owed.push(...typed.slice(got))
    leftover += Math.max(0, got - typed.length)
  })
  const generic = units.filter((unit) => unit.need === null).length
  return { owed, genericOwed: Math.max(0, generic - leftover), waste: Math.max(0, leftover - generic) }
}

/** A resolution the payment so far fits into, with what it leaves owed. */
interface Candidate {
  units: Unit[]
  placement: Placement
}

/** The resolutions in which no mana of the payment goes to waste. */
function candidates(cost: ManaCost, spends: readonly Spend[], life: LifeSymbols): Candidate[] {
  return resolutions(cost, life)
    .map((units) => ({ units, placement: place(cost, units, spends) }))
    .filter((candidate) => candidate.placement.waste === 0)
}

const owedValue = ({ placement }: Candidate): number => placement.owed.length + placement.genericOwed

/** The resolution that leaves the least owed. */
function closest(cost: ManaCost, spends: readonly Spend[], life: LifeSymbols): Candidate | undefined {
  let best: Candidate | undefined
  for (const candidate of candidates(cost, spends, life)) {
    if (best === undefined || owedValue(candidate) < owedValue(best)) best = candidate
  }
  return best
}

/**
 * What is still owed after `spends`: the symbols not yet met, as the cost wrote them (a hybrid symbol stays hybrid
 * until mana of one of its colours meets it), and the generic mana still missing.
 */
export function owed(cost: ManaCost, spends: readonly Spend[], life: LifeSymbols = []): ManaCost {
  const best = closest(cost, spends, life)
  if (best === undefined) return cost.filter((_, index) => !life.includes(index))
  const symbols = [...best.placement.owed]
    .sort((a, b) => a.origin - b.origin)
    .flatMap((unit) => {
      const symbol = cost[unit.origin]
      return symbol === undefined ? [] : [symbol]
    })
  return best.placement.genericOwed > 0 ? [{ Generic: best.placement.genericOwed }, ...symbols] : symbols
}

/** True when `spends` and `life` are a payment the engine accepts: nothing owed, nothing over, exactly the life paid. */
export function covers(cost: ManaCost, spends: readonly Spend[], life: LifeSymbols = []): boolean {
  return candidates(cost, spends, life).some(
    (candidate) => candidate.placement.owed.length === 0 && candidate.placement.genericOwed === 0,
  )
}

/**
 * True when one more `mana` would still go toward the cost. Mana beyond what the colours ask for goes to the generic
 * part, so it fits only while generic mana is owed; anything else is a payment the engine would refuse.
 */
export function fits(
  cost: ManaCost,
  spends: readonly Spend[],
  mana: ManaType,
  life: LifeSymbols = [],
): boolean {
  return candidates(cost, [...spends, { kind: 'pool', mana }], life).length > 0
}

/** The life the payment pays: 2 for each symbol life pays. */
export function lifeOf(life: LifeSymbols): number {
  return life.length * LIFE_PER_SYMBOL
}

/** The payment as the wire takes it: the floating mana to spend, the abilities to tap and the life to pay. */
export function toPayment(spends: readonly Spend[], life: LifeSymbols = []): Payment {
  return {
    pool: spends.flatMap((spend) => (spend.kind === 'pool' ? [spend.mana] : [])),
    taps: spends.flatMap((spend) => (spend.kind === 'tap' ? [spend.option.source] : [])),
    life: lifeOf(life),
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
export function allowed(
  cost: ManaCost,
  pool: ManaPool,
  spends: readonly Spend[],
  spend: Spend,
  life: LifeSymbols = [],
): boolean {
  if (!fits(cost, spends, manaOf(spend), life)) return false
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
  life: LifeSymbols = [],
): ManaOption[] {
  const seen = new Set<ManaType>()
  return abilities.filter((option) => {
    if (seen.has(option.produces) || !allowed(cost, pool, spends, { kind: 'tap', option }, life)) return false
    seen.add(option.produces)
    return true
  })
}

/** A symbol life can pay: whether it does now, and whether that can be changed without breaking the payment. */
export interface LifeChoice {
  /** The symbol's place in the cost. */
  index: number
  paid: boolean
  /** True when the choice can be flipped: the life is there to pay (CR 119.4) and the mana chosen still fits. */
  available: boolean
}

/**
 * The Phyrexian symbols of the cost with what the player may do about each. At most `maxLife` may be paid in all
 * (the most the engine allows, within the life total), and a change that would leave mana with no place is refused.
 */
export function lifeChoices(
  cost: ManaCost,
  spends: readonly Spend[],
  life: LifeSymbols,
  maxLife: number,
): LifeChoice[] {
  return cost.flatMap((symbol, index) => {
    if (!canPayLife(symbol)) return []
    const paid = life.includes(index)
    const after = paid ? life.filter((other) => other !== index) : [...life, index]
    const room = paid || lifeOf(after) <= maxLife
    return [{ index, paid, available: room && candidates(cost, spends, after).length > 0 }]
  })
}
