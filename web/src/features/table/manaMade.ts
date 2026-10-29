/**
 * What a source of mana gives, as one shape for the payment prompt (a `ManaOption`) and for a card on the table (the
 * words of its activated ability): the mana of each way a use can go, what a use costs and whether it sacrifices.
 */
import type { ManaCost, ManaOption, ManaSymbol, ManaType, ObjectView, PlayerId } from '@/protocol/generated'

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
  const ways = distinct(
    options.map((option) => ({
      made: madeByOption(option),
      cost: option.cost ?? [],
      sacrifice: option.sacrifice === true,
    })),
  )
  const any = ways.length > 1 && coversColours(ways)
  const [first] = ways
  return {
    any,
    ways: any ? [] : ways,
    cost: first?.cost ?? [],
    sacrifice: first?.sacrifice ?? false,
  }
}

const SYMBOLS: Record<string, ManaType> = {
  W: 'White',
  U: 'Blue',
  B: 'Black',
  R: 'Red',
  G: 'Green',
  C: 'Colorless',
}

/** The mana of the `{W}{U}` symbols in a piece of wording; empty when there is none or something else is in it. */
function symbolsIn(text: string): ManaType[] {
  const rest = text.replace(/\{([WUBRGC])\}/g, '').replace(/[\s,]|\bor\b/g, '')
  if (rest !== '') return []
  return [...text.matchAll(/\{([WUBRGC])\}/g)].flatMap((found) => {
    const mana = SYMBOLS[found[1] ?? '']
    return mana === undefined ? [] : [mana]
  })
}

/** The ways an effect adds mana, read off `Add {C}{C}.`, `Add {W} or {U}.` or `Add one mana of any color.`; null when it does neither. */
function addedBy(effect: string): { any: boolean; ways: ManaType[][] } | null {
  const sentence = effect.split(/\.\s/)[0]?.replace(/\.$/, '') ?? ''
  const added = /^Add (.+)$/.exec(sentence)?.[1]
  if (added === undefined || /instead/i.test(effect)) return null
  if (/^(one|a) mana of any (one )?colou?r$/i.test(added)) return { any: true, ways: [] }
  const ways = added.split(/\s+or\s+|,\s*/).map(symbolsIn)
  return ways.length === 0 || ways.some((way) => way.length === 0) ? null : { any: false, ways }
}

/** The mana symbols of an ability's cost in words (`{1}, {T}, Sacrifice Treasure`): what it pays besides the tap; null for any other cost item. */
function costOf(cost: string): { mana: ManaCost; sacrifice: boolean; taps: boolean } | null {
  const mana: ManaSymbol[] = []
  let sacrifice = false
  let taps = false
  for (const item of cost.split(/,\s*/)) {
    const generic = /^\{(\d+)\}$/.exec(item)?.[1]
    const colour = /^\{([WUBRGC])\}$/.exec(item)?.[1]
    if (item === '{T}') taps = true
    else if (/^Sacrifice /.test(item)) sacrifice = true
    else if (generic !== undefined) mana.push({ Generic: Number(generic) })
    else if (colour !== undefined && SYMBOLS[colour] !== undefined) mana.push(SYMBOL_OF[SYMBOLS[colour]])
    else return null
  }
  return { mana, sacrifice, taps }
}

/** A mana ability of an object as the strip and the preview read it. */
export interface Ability {
  gives: Gives
  /** True when using it taps its source. */
  taps: boolean
}

/** The mana abilities of an object whose words the client can read, from `activated` or else the printed ones. */
export function manaAbilitiesOf(object: ObjectView): Ability[] {
  const listed = object.activated
  if (listed !== undefined) {
    return listed.flatMap((entry): Ability[] => {
      if (entry.mana !== true) return []
      const cost = costOf(entry.cost)
      const added = addedBy(entry.effect)
      if (cost === null || added === null) return []
      const ways = added.ways.map((made) => ({ made, cost: cost.mana, sacrifice: cost.sacrifice }))
      return [
        { taps: cost.taps, gives: { any: added.any, ways, cost: cost.mana, sacrifice: cost.sacrifice } },
      ]
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
