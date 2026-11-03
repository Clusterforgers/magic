/** Whether mana restricted to some uses may pay for a given spell or ability, as the engine reads it (`spend.rs`). */
import type {
  CardType,
  Color,
  Spend as PaySpend,
  SpendFilter,
  SpendOn,
  SpendTraits,
} from '@/protocol/generated'

const CARD_TYPES: readonly CardType[] = [
  'Artifact',
  'Battle',
  'Creature',
  'Enchantment',
  'Instant',
  'Kindred',
  'Land',
  'Planeswalker',
  'Sorcery',
]
const COLORS: readonly Color[] = ['White', 'Blue', 'Black', 'Red', 'Green']

/** True when the wire's type set (one bit per type, in the order of `CARD_TYPES`) holds the type. */
const hasType = (types: number, type: CardType): boolean => (types >> CARD_TYPES.indexOf(type)) % 2 === 1
const hasColor = (colors: number, color: Color): boolean => (colors >> COLORS.indexOf(color)) % 2 === 1

/** True when something with these traits is of the kind the filter lists; a filter on a chosen type holds for nothing. */
export function filterHolds(filter: SpendFilter, traits: SpendTraits): boolean {
  const types = filter.types ?? []
  const without = filter.without ?? []
  const supertypes = filter.supertypes ?? []
  const subtypes = filter.subtypes ?? []
  const colors = filter.colors ?? []
  return (
    filter.chosen_type !== true &&
    (types.length === 0 || types.some((type) => hasType(traits.types, type))) &&
    !without.some((type) => hasType(traits.types, type)) &&
    (supertypes.length === 0 || supertypes.some((type) => traits.supertypes?.includes(type) === true)) &&
    (subtypes.length === 0 || subtypes.some((type) => traits.subtypes?.includes(type) === true)) &&
    (colors.length === 0 || colors.some((color) => hasColor(traits.color_set ?? 0, color))) &&
    (filter.multicolored !== true || traits.colors >= 2) &&
    (traits.value ?? 0) >= (filter.min_value ?? 0) &&
    (filter.facts ?? []).every((fact) => {
      switch (fact) {
        case 'XInCost':
          return traits.x_in_cost === true
        case 'NotOwned':
          return traits.not_owned === true
        case 'FromGraveyard':
          return traits.from_graveyard === true
        case 'SharesCommanderType':
          return traits.shares_commander_type === true
      }
    })
  )
}

const restricts = (on: SpendOn): boolean => typeof on === 'string' || !('When' in on)

/**
 * True when mana restricted to `only` may be spent on `spend` (a payment for neither a spell nor an ability when
 * missing): mana with no restriction, or with a rider alone, may be spent on anything.
 */
export function spendAllows(spend: PaySpend | undefined, only: readonly SpendOn[] | undefined): boolean {
  const uses = only ?? []
  if (!uses.some(restricts)) return true
  const kind = spend?.kind ?? 'Other'
  const traits = spend?.traits
  return uses.some((on) => {
    if (on === 'CostWithX') return kind !== 'Other' && spend?.has_x === true
    if ('Spell' in on) return kind === 'Spell' && traits !== undefined && filterHolds(on.Spell, traits)
    if ('Ability' in on) return kind === 'Ability' && traits !== undefined && filterHolds(on.Ability, traits)
    return false
  })
}

/** A use in words, for a button: "creature spells", "abilities", "costs with {X}". */
export function useWords(on: SpendOn): string {
  if (on === 'CostWithX') return 'costs with {X}'
  if ('Ability' in on) return 'abilities'
  if ('Spell' in on) {
    const { types = [], subtypes = [], colors = [] } = on.Spell
    const kinds = [...colors, ...subtypes, ...types].map((word) => word.toLowerCase())
    return `${kinds.join(' or ')} spells`.trim()
  }
  return 'anything'
}

/** What the restricted mana may be spent on, in words. */
export function usesWords(only: readonly SpendOn[]): string {
  const words = only.filter(restricts).map(useWords)
  return words.length === 0 ? 'anything' : words.join(' or ')
}
