/** Pure facts about a card for the table: its row, cost pips, type line, frame colour, status words and spoken label. */
import type {
  AbilityDef,
  CombatView,
  Color,
  CounterKind,
  ManaCost,
  ManaSymbol,
  ManaType,
  ObjectId,
  ObjectView,
} from '@/protocol/generated'
import { LIFE_PER_SYMBOL, shapeOf, symbolText } from './manaSymbols'

/** Where a permanent sits on its controller's battlefield. */
export type RowKind = 'creatures' | 'other' | 'lands'

/** What a card is doing in the current combat. */
export type CombatRole = 'attacking' | 'blocked' | 'blocking'

/** How the decision layer marks a card; the table itself never sets one. */
export type Highlight = 'legal' | 'dimmed' | 'selected'

/** The colour of a card frame: one colour, gold for several, grey for none. */
export type Frame = 'white' | 'blue' | 'black' | 'red' | 'green' | 'gold' | 'colorless'

/** The look of a mana symbol. */
export type PipTone = Frame | 'generic'

/** One mana symbol of a cost. */
export interface ManaPip {
  key: string
  /** The fill, or the upper left half of a split symbol. */
  tone: PipTone
  /** The fill of the lower right half of a split symbol: hybrid and monocolored hybrid. */
  split?: PipTone
  /** The letter, number or phi drawn in the symbol; for a split symbol, the one in its upper left half. */
  text: string
  /** The letter in the lower right half of a split symbol. */
  text2?: string
  /** True for a Phyrexian symbol, which carries the phi mark. */
  phyrexian?: boolean
  /** The symbol as a card prints it, without braces: "G", "2", "W/P". */
  symbol: string
  /** The symbol in words, for screen readers. */
  name: string
}

/** A creature is in the creature row even when it is also a land or an artifact; lands have a row; the rest share one. */
export function rowOf(object: ObjectView): RowKind {
  if (object.types.includes('Creature')) return 'creatures'
  if (object.types.includes('Land')) return 'lands'
  return 'other'
}

/** The combat role of `id`, or null when it takes no part. */
export function combatRoleOf(combat: CombatView, id: ObjectId): CombatRole | null {
  for (const attacker of combat.attackers) {
    if (attacker.object === id) return attacker.blocked ? 'blocked' : 'attacking'
    if (attacker.blockers.includes(id)) return 'blocking'
  }
  return null
}

const COLORS: readonly Color[] = ['White', 'Blue', 'Black', 'Red', 'Green']
const FRAME_OF: Record<Color, Frame> = {
  White: 'white',
  Blue: 'blue',
  Black: 'black',
  Red: 'red',
  Green: 'green',
}
const LETTER: Record<ManaType, string> = {
  White: 'W',
  Blue: 'U',
  Black: 'B',
  Red: 'R',
  Green: 'G',
  Colorless: 'C',
}

/** The letter that stands for a kind of mana: W, U, B, R, G or C. */
export function manaLetter(mana: ManaType): string {
  return LETTER[mana]
}

/** The colours in a colour set, which the wire sends as a bit mask in WUBRG order. */
export function colorsInSet(set: number): Color[] {
  return COLORS.filter((_, index) => (set & (1 << index)) !== 0)
}

/** The frame colour of a card from its current colours. */
export function frameOf(object: ObjectView): Frame {
  const [first, second] = object.colors
  if (first === undefined) return 'colorless'
  return second === undefined ? FRAME_OF[first] : 'gold'
}

/** The Phyrexian mark drawn over a symbol that life can pay. */
export const PHI = 'ϕ'

const colorWord = (color: Color): string => color.toLowerCase()

function pipOf(symbol: ManaSymbol, index: number): ManaPip {
  const shape = shapeOf(symbol)
  const base = { key: `${index}-${symbolText(symbol)}`, symbol: symbolText(symbol) }
  switch (shape.kind) {
    case 'generic':
      return { ...base, tone: 'generic', text: String(shape.count), name: `${shape.count} generic` }
    case 'colored':
      return { ...base, tone: FRAME_OF[shape.color], text: LETTER[shape.color], name: colorWord(shape.color) }
    case 'colorless':
      return { ...base, tone: 'colorless', text: LETTER.Colorless, name: 'colorless' }
    case 'hybrid': {
      const [a, b] = shape.colors
      return {
        ...base,
        tone: FRAME_OF[a],
        split: FRAME_OF[b],
        text: LETTER[a],
        text2: LETTER[b],
        name: `${colorWord(a)} or ${colorWord(b)} mana`,
      }
    }
    case 'monoHybrid':
      return {
        ...base,
        tone: 'generic',
        split: FRAME_OF[shape.color],
        text: '2',
        text2: LETTER[shape.color],
        name: `${colorWord(shape.color)} mana or 2 generic mana`,
      }
    case 'phyrexian':
      return {
        ...base,
        tone: FRAME_OF[shape.color],
        text: PHI,
        phyrexian: true,
        name: `${colorWord(shape.color)} mana or ${LIFE_PER_SYMBOL} life`,
      }
    case 'hybridPhyrexian': {
      const [a, b] = shape.colors
      return {
        ...base,
        tone: FRAME_OF[a],
        split: FRAME_OF[b],
        text: PHI,
        phyrexian: true,
        name: `${colorWord(a)} or ${colorWord(b)} mana or ${LIFE_PER_SYMBOL} life`,
      }
    }
    case 'colorlessHybrid':
      return {
        ...base,
        tone: 'colorless',
        split: FRAME_OF[shape.color],
        text: LETTER.Colorless,
        text2: LETTER[shape.color],
        name: `colorless or ${colorWord(shape.color)} mana`,
      }
    case 'colorlessPhyrexian':
      return {
        ...base,
        tone: 'colorless',
        text: PHI,
        phyrexian: true,
        name: `colorless mana or ${LIFE_PER_SYMBOL} life`,
      }
  }
}

/** The symbols of a mana cost in card order, as the server sends them; a free cost is one zero. */
export function costPips(cost: ManaCost): ManaPip[] {
  const pips = cost.map(pipOf)
  return pips.length > 0 ? pips : [{ key: 'zero', tone: 'generic', text: '0', symbol: '0', name: 'zero' }]
}

/** A cost as written on a card, such as '{2}{U}{R}' or '{2}{W/P}'. */
export function costText(cost: ManaCost): string {
  return costPips(cost)
    .map((pip) => `{${pip.symbol}}`)
    .join('')
}

/** "Legendary Creature — Elf Warrior". */
export function typeLine(object: ObjectView): string {
  const kinds = [...object.supertypes, ...object.types].join(' ')
  return object.subtypes.length === 0 ? kinds : `${kinds} — ${object.subtypes.join(' ')}`
}

/** "2/3" for a card with power and toughness, else null. */
export function powerToughness(object: ObjectView): string | null {
  return object.power === null || object.toughness === null ? null : `${object.power}/${object.toughness}`
}

/** The names of an object's keywords in the order the server sends them; none for a card that has none. */
export function keywordNames(object: ObjectView): readonly string[] {
  return object.keywords ?? []
}

/** "Flying, First strike" for a card with keywords, else null. */
export function keywordLine(object: ObjectView): string | null {
  const names = keywordNames(object)
  return names.length === 0 ? null : names.join(', ')
}

/** A kind of counter by its name: "+1/+1", "loyalty", "charge". */
export function counterName(kind: CounterKind): string {
  return typeof kind === 'string' ? kind : kind.Other
}

const countersOf = (object: ObjectView): readonly (readonly [CounterKind, number])[] => object.counters ?? []

/** The loyalty of a planeswalker, which its face shows as a number of its own, or null when it has none. */
export function loyaltyOf(object: ObjectView): number | null {
  const found = countersOf(object).find(([kind]) => kind === 'loyalty')
  return found === undefined ? null : found[1]
}

/** The counters a face shows as badges, such as "+1/+1 ×2" and "charge"; loyalty is not among them. */
export function counterLabels(object: ObjectView): string[] {
  return countersOf(object)
    .filter(([kind]) => kind !== 'loyalty')
    .map(([kind, count]) => (count === 1 ? counterName(kind) : `${counterName(kind)} ×${count}`))
}

/** The same counters in words, loyalty included, for a screen reader: "2 +1/+1 counters", "loyalty 3". */
export function counterWords(object: ObjectView): string[] {
  return countersOf(object).map(([kind, count]) =>
    kind === 'loyalty'
      ? `loyalty ${count}`
      : `${count} ${counterName(kind)} ${count === 1 ? 'counter' : 'counters'}`,
  )
}

/** What a permanent is attached to and what is attached to it, by name. */
export interface Links {
  /** The permanent an Aura or Equipment is attached to. */
  host?: string | undefined
  /** The permanents attached to it. */
  attached?: readonly string[] | undefined
}

/** "attached to Grizzly Bears" for an Aura or Equipment; "Pacifism and Bonesplitter attached" for its host. */
export function linkWords({ host, attached = [] }: Links): string[] {
  const words: string[] = []
  if (host !== undefined) words.push(`attached to ${host}`)
  if (attached.length > 0) words.push(`${attached.join(' and ')} attached`)
  return words
}

/** One ability as the rules text prints it. */
export function abilityText(ability: AbilityDef): string {
  return `{T}: Add {${LETTER[ability.Mana.produces]}}.`
}

/** One ability in words, for a screen reader. */
export function abilityWords(ability: AbilityDef): string {
  return `Tap: add ${ability.Mana.produces.toLowerCase()} mana`
}

/** The short labels of a permanent's state, as drawn on the card face. */
export function statusLabels(object: ObjectView, role: CombatRole | null): string[] {
  const labels: string[] = []
  const state = object.permanent
  if (state?.tapped === true) labels.push('Tapped')
  if (state?.summoning_sick === true) labels.push('Sick')
  if (state !== null && state.damage > 0) labels.push(`${state.damage} dmg`)
  if (role === 'attacking') labels.push('Attacking')
  if (role === 'blocked') labels.push('Blocked')
  if (role === 'blocking') labels.push('Blocking')
  return labels
}

/** The same state in full words, for a screen reader and the preview. */
export function statusWords(object: ObjectView, role: CombatRole | null): string[] {
  const words: string[] = []
  const state = object.permanent
  if (state?.tapped === true) words.push('tapped')
  if (state?.summoning_sick === true) words.push('summoning sick')
  if (state !== null && state.damage > 0) words.push(`${state.damage} damage`)
  if (role === 'attacking') words.push('attacking')
  if (role === 'blocked') words.push('attacking, blocked')
  if (role === 'blocking') words.push('blocking')
  return words
}

/** Everything a screen reader needs from a card face. */
export function cardLabel(object: ObjectView, role: CombatRole | null, links: Links = {}): string {
  const parts = [object.name]
  if (object.mana_cost !== null) {
    parts.push(
      `cost ${costPips(object.mana_cost)
        .map((pip) => pip.name)
        .join(', ')}`,
    )
  }
  parts.push(typeLine(object))
  if (object.power !== null && object.toughness !== null) {
    parts.push(`power ${object.power}, toughness ${object.toughness}`)
  }
  parts.push(...keywordNames(object).map((name) => name.toLowerCase()))
  parts.push(...object.abilities.map(abilityWords))
  parts.push(...statusWords(object, role))
  parts.push(...counterWords(object), ...linkWords(links))
  return parts.join(', ')
}
