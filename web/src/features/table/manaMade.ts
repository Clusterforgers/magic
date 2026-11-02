/**
 * What a source of mana gives, as one shape for the payment prompt (a `ManaOption`) and for a card on the table (the
 * ways of its mana abilities in `ObjectView.activated`): the mana of each way a use can go, what a use costs and whether
 * it sacrifices.
 */
import type {
  ManaCost,
  ManaOption,
  ManaSymbol,
  ManaType,
  ManaWayView,
  ObjectView,
  PlayerId,
} from '@/protocol/generated'

/** The five colours as a pool counts them. */
export const COLOURS: readonly ManaType[] = ['White', 'Blue', 'Black', 'Red', 'Green']

/** One way a use of a source can go: the mana it makes in order, and what it takes. */
export interface Way {
  made: readonly ManaType[]
  /** Mana the use costs (a Signet's `{1}`); empty for none. */
  cost: ManaCost
  /** True when the use sacrifices its source (a Treasure). */
  sacrifice: boolean
}

/** What a source gives: one mana of any colour, or the listed ways. */
export interface Gives {
  any: boolean
  ways: readonly Way[]
  cost: ManaCost
  sacrifice: boolean
}

const SYMBOL_OF: Record<ManaType, ManaSymbol> = {
  White: { Colored: 'White' },
  Blue: { Colored: 'Blue' },
  Black: { Colored: 'Black' },
  Red: { Colored: 'Red' },
  Green: { Colored: 'Green' },
  Colorless: 'Colorless',
}

/** Mana as the symbols of a cost, so the cost pips draw it: `[White, Colorless]` is `{W}{C}`. */
export function madeCost(made: readonly ManaType[]): ManaCost {
  return made.map((mana) => SYMBOL_OF[mana])
}

/** The mana one use of the option makes: `count` of what it produces (Sol Ring: two), then what `also` adds. */
export function madeByOption(option: ManaOption): ManaType[] {
  return [...Array.from({ length: option.count ?? 1 }, () => option.produces), ...(option.also ?? [])]
}

const keyOf = (way: Way): string =>
  `${way.made.join('')}|${JSON.stringify(way.cost)}|${way.sacrifice ? 's' : ''}`

/** The distinct ways, in order of first appearance. */
function distinct(ways: readonly Way[]): Way[] {
  const seen = new Set<string>()
  return ways.filter((way) => {
    const key = keyOf(way)
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

/** True when the ways are one mana each and between them make every colour: the source makes one of any colour. */
function coversColours(ways: readonly Way[]): boolean {
  return COLOURS.every((colour) => ways.some((way) => way.made.length === 1 && way.made[0] === colour))
}

/** What the options of one permanent give; a source with an option for each colour gives one mana of any colour. */
export function givesOfOptions(options: readonly ManaOption[]): Gives {
  return givesOfWays(
    options.map((option) => ({
      made: madeByOption(option),
      cost: option.cost ?? [],
      sacrifice: option.sacrifice === true,
    })),
  )
}

/** One way of a mana ability as the view sends it (`ActivatedView.ways`). */
export function wayOfView(way: ManaWayView): Way {
  return { made: way.mana, cost: way.cost ?? [], sacrifice: way.sacrifice === true }
}

/** What the ways of one ability give; a way for each colour gives one mana of any colour. */
function givesOfWays(all: readonly Way[]): Gives {
  const ways = distinct(all)
  const any = ways.length > 1 && coversColours(ways)
  const [first] = ways
  return {
    any,
    ways: any ? [] : ways,
    cost: first?.cost ?? [],
    sacrifice: first?.sacrifice ?? false,
  }
}

/** A mana ability of an object as the strip and the preview read it. */
export interface Ability {
  gives: Gives
  /** True when using it taps its source. */
  taps: boolean
}

/**
 * The mana abilities of an object that its controller can use now, from the ways of `activated` or else the printed
 * ones.
 */
export function manaAbilitiesOf(object: ObjectView): Ability[] {
  const listed = object.activated
  if (listed !== undefined) {
    return listed.flatMap((entry): Ability[] => {
      const [first] = entry.ways ?? []
      if (entry.mana !== true || first === undefined) return []
      return [{ taps: first.tapless !== true, gives: givesOfWays((entry.ways ?? []).map(wayOfView)) }]
    })
  }
  return object.abilities.flatMap((ability): Ability[] =>
    'Mana' in ability
      ? [
          {
            taps: true,
            gives: {
              any: false,
              ways: [{ made: [ability.Mana.produces], cost: [], sacrifice: false }],
              cost: [],
              sacrifice: false,
            },
          },
        ]
      : [],
  )
}

/** All the abilities folded into one: any colour if one ability gives it, else every way, once. */
export function foldGives(abilities: readonly Ability[]): Gives | null {
  const [first] = abilities
  if (first === undefined) return null
  const ways = distinct(abilities.flatMap((ability) => ability.gives.ways))
  return {
    any: abilities.some((ability) => ability.gives.any) || coversColours(ways),
    ways,
    cost: first.gives.cost,
    sacrifice: first.gives.sacrifice,
  }
}

/** True when the seat may tap the permanent for mana now: theirs, untapped, not a creature that came under their control this turn. */
export function mayTap(object: ObjectView, seat: PlayerId | null): boolean {
  const permanent = object.permanent
  if (seat === null || permanent === null || object.zone !== 'Battlefield') return false
  if (object.controller !== seat || object.face_down === true || permanent.tapped) return false
  return !(object.types.includes('Creature') && permanent.summoning_sick)
}

/** What a tap of the permanent gives the seat now, or null when it cannot be tapped for mana (or the client cannot read the ability). */
export function givesNow(object: ObjectView, seat: PlayerId | null): Gives | null {
  if (!mayTap(object, seat)) return null
  const abilities = manaAbilitiesOf(object).filter((ability) => ability.taps)
  const gives = foldGives(abilities)
  return gives === null || (!gives.any && gives.ways.length === 0) ? null : gives
}
