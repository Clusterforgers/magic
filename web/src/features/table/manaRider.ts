/**
 * Mana that carries a rider: "When that mana is spent to cast a creature spell that shares a creature type with your
 * commander, scry 1" (Path of Ancestry). The rider sets something off when the mana pays for a spell; it limits nothing, so
 * such mana is plain mana with a mark, and only mana whose `only` also lists a use is restricted.
 */
import type {
  ManaPool as ManaPoolValue,
  ManaType,
  ObjectView,
  Restricted,
  SpendFact,
  SpendFilter,
  SpendOn,
  WhenSpent,
} from '@/protocol/generated'

// The order of `ManaPool.counts` on the wire.
const TYPES: readonly ManaType[] = ['White', 'Blue', 'Black', 'Red', 'Green', 'Colorless']

/** The rider a use of mana is, or none for a use that limits what the mana may pay for. */
const riderOf = (on: SpendOn): WhenSpent | undefined =>
  typeof on === 'object' && 'When' in on ? on.When : undefined

/** Whether the mana of this unit may pay for some things only: a use besides its riders. */
export const limits = (unit: Restricted): boolean => unit.only.some((on) => riderOf(on) === undefined)

/** Mana that carries a rider and no restriction: how many of a kind, and the riders it carries, each once. */
export interface RiderMana {
  type: ManaType
  count: number
  riders: WhenSpent[]
}

/** The mana of a pool that carries riders alone (it can pay for anything), by kind in the order of the wire. */
export function manaRiders(pool: ManaPoolValue): RiderMana[] {
  const units = (pool.restricted ?? []).filter((unit) => unit.only.length > 0 && !limits(unit))
  return TYPES.flatMap((type) => {
    const mine = units.filter((unit) => unit.mana === type)
    if (mine.length === 0) return []
    const riders: WhenSpent[] = []
    for (const rider of mine.flatMap((unit) => unit.only.flatMap((on) => riderOf(on) ?? []))) {
      if (!riders.some((seen) => JSON.stringify(seen) === JSON.stringify(rider))) riders.push(rider)
    }
    return [{ type, count: mine.length, riders }]
  })
}

const FACTS: Record<SpendFact, string> = {
  XInCost: 'with {X} in its cost',
  NotOwned: 'you do not own',
  FromGraveyard: 'cast from a graveyard',
  SharesCommanderType: 'that shares a creature type with your commander',
}

const lower = (text: string): string => text.toLowerCase()

/** "a creature spell that shares a creature type with your commander": the spells a rider waits for. */
export function spendFilterWords(filter: SpendFilter): string {
  const colours = (filter.colors ?? []).map(lower).join(' or ')
  // Every part that is listed holds, and a part holds when any of its words does: "an elf creature", "an artifact or creature".
  const kinds = [filter.subtypes ?? [], filter.types ?? []]
    .filter((part) => part.length > 0)
    .map((part) => part.map(lower).join(' or '))
    .join(' ')
  const without = (filter.without ?? []).map((kind) => `non${lower(kind)}`).join(' ')
  const head = [
    (filter.supertypes ?? []).map(lower).join(' or '),
    without,
    filter.multicolored === true ? 'multicolored' : '',
    colours,
    filter.chosen_type === true ? 'chosen-type' : '',
    kinds,
    'spell',
  ].filter((part) => part !== '')
  const value = (filter.min_value ?? 0) > 0 ? [`with mana value ${filter.min_value} or greater`] : []
  const tail = [...value, ...(filter.facts ?? []).map((fact) => FACTS[fact])]
  const noun = head.join(' ')
  return [`${/^[aeiou]/.test(noun) ? 'an' : 'a'} ${noun}`, ...tail].join(' ')
}

/** The sentence of a rider in the source's own mana ability, when the source words it ("When that mana is spent ..."). */
function printedRider(source: ObjectView | undefined): string | undefined {
  for (const entry of source?.activated ?? []) {
    if (entry.mana !== true) continue
    const at = entry.effect.indexOf('When that mana is spent')
    if (at >= 0) return entry.effect.slice(at)
  }
  return undefined
}

/**
 * What a rider does, in words: the card's own sentence when the source's mana ability words it, else the kind of spell it
 * waits for and the permanent that made the mana.
 */
export function riderWords(rider: WhenSpent, source: ObjectView | undefined): string {
  return (
    printedRider(source) ??
    `When this mana is spent to cast ${spendFilterWords(rider.spell)}, an ability of ${source?.name ?? 'the permanent that made it'} triggers.`
  )
}

/** Tells one rider from another: they are plain data. */
export const riderKey = (rider: WhenSpent): string => JSON.stringify(rider)
