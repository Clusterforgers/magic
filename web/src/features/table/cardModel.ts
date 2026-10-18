/** Pure facts about a card for the table: its row, cost pips, type line, frame colour, status words and spoken label. */
import type {
  AbilityDef,
  CombatView,
  Color,
  ManaCost,
  ManaType,
  ObjectId,
  ObjectView,
} from '@/protocol/generated'

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
  tone: PipTone
  /** The letter or number drawn in the symbol. */
  text: string
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

/** The symbols of a mana cost in card order: the generic amount, colourless, then white to green. */
export function costPips(cost: ManaCost): ManaPip[] {
  const pips: ManaPip[] = []
  if (cost.generic > 0) {
    pips.push({
      key: 'generic',
      tone: 'generic',
      text: String(cost.generic),
      name: `${cost.generic} generic`,
    })
  }
  for (let n = 0; n < cost.colorless; n += 1) {
    pips.push({ key: `colorless${n}`, tone: 'colorless', text: LETTER.Colorless, name: 'colorless' })
  }
  cost.colored.forEach((count, index) => {
    const color = COLORS[index]
    if (color === undefined) return
    for (let n = 0; n < count; n += 1) {
      pips.push({
        key: `${color}${n}`,
        tone: FRAME_OF[color],
        text: LETTER[color],
        name: color.toLowerCase(),
      })
    }
  })
  return pips.length > 0 ? pips : [{ key: 'zero', tone: 'generic', text: '0', name: 'zero' }]
}

/** A cost as written on a card, such as "{2}{U}{R}". */
export function costText(cost: ManaCost): string {
  return costPips(cost)
    .map((pip) => `{${pip.text}}`)
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
export function cardLabel(object: ObjectView, role: CombatRole | null): string {
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
  parts.push(...object.abilities.map(abilityWords))
  parts.push(...statusWords(object, role))
  return parts.join(', ')
}
