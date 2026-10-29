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

/** Mana by kind, in `MANA_TYPES` order. */
type Counts = number[]

const kindAt = (mana: ManaType): number => MANA_TYPES.indexOf(mana)

/** The mana one use of a source adds: `count` of what it produces (Sol Ring adds 2), then what `also` adds. */
export function madeBy(option: ManaOption): ManaType[] {
  return [...Array.from({ length: option.count ?? 1 }, () => option.produces), ...(option.also ?? [])]
}

/** The mana a piece of the payment makes. */
export function madeOf(spend: Spend): ManaType[] {
  return spend.kind === 'pool' ? [spend.mana] : madeBy(spend.option)
}

/** True when using the source costs mana (a Signet's `{1}`, a filter land's `{W/U}`), paid from mana made before it. */
export function costsMana(option: ManaOption): boolean {
  return (option.cost?.length ?? 0) > 0
}

/** How many of each kind of mana the payment makes, in `MANA_TYPES` order, before any source's own cost is paid. */
export function tally(spends: readonly Spend[]): Counts {
  const counts = MANA_TYPES.map(() => 0)
  for (const mana of spends.flatMap(madeOf)) counts[kindAt(mana)] = (counts[kindAt(mana)] ?? 0) + 1
  return counts
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

/** What a plain cost asks once its choices are made: mana of each kind and more mana of any kind. */
interface Ask {
  typed: Counts
  any: number
}

/** Every way to pay the mana of `cost` (life never pays a source's own cost). */
function asksOf(cost: ManaCost): Ask[] {
  return resolutions(cost).map((units) => {
    const typed = MANA_TYPES.map(() => 0)
    for (const unit of units) {
      if (unit.need !== null) typed[kindAt(unit.need)] = (typed[kindAt(unit.need)] ?? 0) + 1
    }
    return { typed, any: units.filter((unit) => unit.need === null).length }
  })
}

/** Every way to take `ask` out of `free`: its kinds first, then its generic part from any kinds. */
function takeWays(free: Counts, ask: Ask): Counts[] {
  const left = free.map((have, at) => have - (ask.typed[at] ?? 0))
  if (left.some((have) => have < 0)) return []
  const found: Counts[] = []
  const walk = (at: number, need: number, now: Counts): void => {
    if (need === 0) {
      found.push(now)
      return
    }
    if (at >= now.length) return
    for (let take = Math.min(need, now[at] ?? 0); take >= 0; take -= 1) {
      walk(
        at + 1,
        need - take,
        now.map((have, kind) => (kind === at ? have - take : have)),
      )
    }
  }
  walk(0, ask.any, left)
  return found
}

const sourceKey = (option: ManaOption): string => `${option.source.object}:${option.source.ability}`

/**
 * The mana the payment leaves for the cost, once for every way to pay the sources that cost mana from mana made
 * before them (CR 605.3b), as the engine reads a payment (`mana_cost.rs`): floating mana and the other sources come
 * first, the ones that cost mana in any order. Empty when no order pays them all.
 */
export function netMana(spends: readonly Spend[]): Counts[] {
  const costly = spends.flatMap((spend) =>
    spend.kind === 'tap' && costsMana(spend.option) ? [spend.option] : [],
  )
  const plain = spends.filter((spend) => spend.kind === 'pool' || !costsMana(spend.option))
  const seen = new Set<string>()
  const found = new Map<string, Counts>()
  const walk = (now: Counts, rest: readonly ManaOption[]): void => {
    const key = `${now.join()}|${rest.map(sourceKey).join()}`
    if (seen.has(key)) return
    seen.add(key)
    if (rest.length === 0) {
      found.set(now.join(), now)
      return
    }
    rest.forEach((option, at) => {
      const others = rest.filter((_, other) => other !== at)
      for (const ask of asksOf(option.cost ?? [])) {
        for (const after of takeWays(now, ask)) {
          const next = [...after]
          for (const mana of madeBy(option)) next[kindAt(mana)] = (next[kindAt(mana)] ?? 0) + 1
          walk(next, others)
        }
      }
    })
  }
  walk(tally(plain), costly)
  return [...found.values()]
}

/** True when the mana `spends` make, after their own costs, can pay the mana cost of using `option` now. */
export function canFund(spends: readonly Spend[], option: ManaOption): boolean {
  if (!costsMana(option)) return true
  const asks = asksOf(option.cost ?? [])
  return netMana(spends).some((free) => asks.some((ask) => takeWays(free, ask).length > 0))
}

/** What the units still ask for after the payment: the kinds not met, the generic part not met, and mana with no place. */
interface Placement {
  owed: Unit[]
  genericOwed: number
  waste: number
}

/** Puts the payment's mana against the units: each kind meets its own, and what is left over goes to generic. */
function place(cost: ManaCost, units: readonly Unit[], have: Counts): Placement {
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

/** The resolutions, for each way the payment funds its sources, in which no mana of the payment goes to waste. */
function candidates(cost: ManaCost, spends: readonly Spend[], life: LifeSymbols): Candidate[] {
  const ways = resolutions(cost, life)
  return netMana(spends)
    .flatMap((have) => ways.map((units) => ({ units, placement: place(cost, units, have) })))
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
  if (!exact(cost, spends, life)) return false
  // With a source that costs mana the engine also refuses a part that could be left out (`mana_cost::spent`).
  if (!spends.some((spend) => spend.kind === 'tap' && costsMana(spend.option))) return true
  return spends.every((_, at) => !exact(cost, spends.filter((__, other) => other !== at), life))
}

/** True when some way of reading the payment meets the cost with nothing owed and nothing over. */
function exact(cost: ManaCost, spends: readonly Spend[], life: LifeSymbols): boolean {
  return candidates(cost, spends, life).some(
    (candidate) => candidate.placement.owed.length === 0 && candidate.placement.genericOwed === 0,
  )
}

/** The most payments `completable` looks at before it gives the benefit of the doubt. */
const SEARCH_BUDGET = 4_000

/** True when the source may be used by hand: its mana has no restriction, which the prompt does not read yet. */
export function usableByHand(option: ManaOption): boolean {
  return (option.only?.length ?? 0) === 0
}

/** What is left to finish a payment with: every source of the decision and the most life it may take. */
export interface Reach {
  sources: readonly ManaOption[]
  maxLife: number
}

/** `life` and every larger set of Phyrexian symbols paid with life within `maxLife`, the smallest first. */
function lifeSupersets(cost: ManaCost, life: LifeSymbols, maxLife: number): LifeSymbols[] {
  const open = cost.flatMap((symbol, index) => (canPayLife(symbol) && !life.includes(index) ? [index] : []))
  let found: LifeSymbols[] = [life]
  for (const index of open) found = [...found, ...found.map((set) => [...set, index])]
  return found.filter((set) => lifeOf(set) <= Math.max(maxLife, lifeOf(life))).sort((a, b) => a.length - b.length)
}

/**
 * True when more of the floating mana, the sources of `reach` not used yet and life for more Phyrexian symbols (within
 * `maxLife`) can finish the payment so the engine accepts it. Each permanent is used once; additions are tried in one
 * order (a set, not a sequence), and a payment with waste goes on only while a source that costs mana could still
 * take the extra mana.
 */
export function completable(
  cost: ManaCost,
  pool: ManaPool,
  spends: readonly Spend[],
  life: LifeSymbols,
  reach: Reach,
): boolean {
  // The sources that cost mana come last, so the ones that fund them are tried before them.
  const options = reach.sources
    .filter(usableByHand)
    .sort((a, b) => Number(costsMana(a)) - Number(costsMana(b)))
  let budget = SEARCH_BUDGET
  const walk = (paid: LifeSymbols, now: readonly Spend[], from: number, mana: number): boolean => {
    budget -= 1
    if (budget < 0) return true
    if (covers(cost, now, paid)) return true
    const used = new Set(now.flatMap((spend) => (spend.kind === 'tap' ? [spend.option.source.object] : [])))
    const sinks = options.some((option) => costsMana(option) && !used.has(option.source.object))
    if (!sinks && candidates(cost, now, paid).length === 0) return false
    for (let kind = mana; kind < MANA_TYPES.length; kind += 1) {
      const type = MANA_TYPES[kind]
      if (type !== undefined && poolLeft(pool, now, type) > 0) {
        if (walk(paid, [...now, { kind: 'pool', mana: type }], 0, kind)) return true
      }
    }
    for (let at = from; at < options.length; at += 1) {
      const option = options[at]
      if (option === undefined || used.has(option.source.object) || !canFund(now, option)) continue
      if (walk(paid, [...now, { kind: 'tap', option }], at + 1, MANA_TYPES.length)) return true
    }
    return false
  }
  return lifeSupersets(cost, life, reach.maxLife).some((paid) => walk(paid, spends, 0, 0))
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
 * True when `spend` may join the payment: floating mana is left for it, a permanent is tapped once (the engine would
 * tap it again for a second ability), a source that costs mana comes after the mana that pays for it, and nothing
 * is wasted. With `reach` (every source of the decision) "nothing is wasted" means the payment can still be
 * finished from what is left (`completable`); without it, that all the mana chosen has a place in the cost.
 */
export function allowed(
  cost: ManaCost,
  pool: ManaPool,
  spends: readonly Spend[],
  spend: Spend,
  life: LifeSymbols = [],
  reach?: Reach,
): boolean {
  if (spend.kind === 'pool' && poolLeft(pool, spends, spend.mana) <= 0) return false
  if (spend.kind === 'tap') {
    const object = spend.option.source.object
    if (!usableByHand(spend.option)) return false
    if (spends.some((other) => other.kind === 'tap' && other.option.source.object === object)) return false
    if (!canFund(spends, spend.option)) return false
  }
  const next = [...spends, spend]
  return reach === undefined
    ? candidates(cost, next, life).length > 0
    : completable(cost, pool, next, life, reach)
}

/** What tells two abilities of one permanent apart for a payment: the mana they make and what they cost. */
const optionKey = (option: ManaOption): string =>
  `${madeBy(option).join()}|${JSON.stringify(option.cost ?? [])}|${String(option.sacrifice ?? false)}`

/** The options of one permanent that may join the payment, one per kind of use (two abilities alike are one). */
export function fittingOptions(
  cost: ManaCost,
  pool: ManaPool,
  spends: readonly Spend[],
  abilities: readonly ManaOption[],
  life: LifeSymbols = [],
  reach?: Reach,
): ManaOption[] {
  const seen = new Set<string>()
  return abilities.filter((option) => {
    const key = optionKey(option)
    if (seen.has(key) || !allowed(cost, pool, spends, { kind: 'tap', option }, life, reach)) return false
    seen.add(key)
    return true
  })
}

/**
 * The payment after `spends` lose the one at `index`, and with it every source that costs mana and can no longer be
 * paid for by the mana chosen before it.
 */
export function withoutSpend(spends: readonly Spend[], index: number): Spend[] {
  const kept: Spend[] = []
  spends.forEach((spend, at) => {
    if (at === index) return
    if (spend.kind === 'tap' && !canFund(kept, spend.option)) return
    kept.push(spend)
  })
  return kept
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
