/**
 * The abilities a Priority prompt lets the viewer activate from the table: which permanents and cards of the hand, the
 * graveyard, the exile and the command zone carry one, what each is called, and the plays a card with several offers in a chooser.
 * Mana abilities are never listed here: they stay the business of `PriorityOptions.mana_abilities` and are no play
 * (docs/arena-flow.md).
 */
import type {
  AbilityRef,
  ActivatedView,
  ManaOption,
  ObjectId,
  ObjectView,
  PlayerId,
  PriorityAction,
  PriorityOptions,
} from '@/protocol/generated'
import { givesOfOptions } from '../manaMade'
import { madeWords } from '../ManaMade'
import type { CardNote, CardNotes } from './marks'

/** One thing a click on a card can do, as a line of the chooser. */
export interface Play {
  /** Tells the plays of one card apart. */
  key: string
  /** The line: the keyword (`Equip {1}`) or the cost and the effect, as the card words them. */
  label: string
  /** The effect under a keyword's line. */
  detail?: string
  /** The spoken name. */
  words: string
  /** What the prompt answers when it is chosen. */
  action: PriorityAction
}

/** The plays of each card that has an ability to activate, in the order the offer lists them. */
export type Activations = ReadonlyMap<ObjectId, readonly Play[]>

const NONE: Activations = new Map()

const SPOKEN: Readonly<Record<string, string>> = {
  T: 'tap',
  Q: 'untap',
  W: 'white',
  U: 'blue',
  B: 'black',
  R: 'red',
  G: 'green',
  C: 'colorless',
  S: 'snow',
  E: 'energy',
  P: 'life',
}

/** Oracle wording read aloud: `{2}, {T}` is "2, tap", `{3}{B}` "3 black" and `{W/P}` "white or life". */
export function spoken(text: string): string {
  return text.replace(/\}\{/g, '} {').replace(/\{([^}]+)\}/g, (_all, symbol: string) =>
    symbol
      .split('/')
      .map((part) => SPOKEN[part] ?? part)
      .join(' or '),
  )
}

/** Tells the abilities of one card apart: their place on the card and where they come from. */
export const abilityKey = (ref: AbilityRef): string =>
  `ability-${ref.ability}-${JSON.stringify(ref.source ?? null)}`

const sameAbility = (a: AbilityRef, b: AbilityRef): boolean =>
  a.object === b.object &&
  a.ability === b.ability &&
  JSON.stringify(a.source ?? 'Printed') === JSON.stringify(b.source ?? 'Printed')

/**
 * Whether `seat` may activate abilities of `object` from where it lies: a permanent it controls, or a card of its own
 * hand (cycling, channel), graveyard, exile or command zone.
 */
function usable(object: ObjectView, seat: PlayerId): boolean {
  const { zone } = object
  if (zone === 'Battlefield') return object.controller === seat
  if (zone === 'Exile') return object.owner === seat
  if (typeof zone === 'object') {
    if ('Hand' in zone) return zone.Hand === seat
    if ('Graveyard' in zone) return zone.Graveyard === seat
    if ('Command' in zone) return zone.Command === seat
  }
  return false
}

/** The line and the effect of an ability in the card's words; a placeholder where the server gave none. */
function wording(ref: AbilityRef, worded: ActivatedView | undefined): Pick<Play, 'label' | 'detail'> {
  if (worded === undefined) return { label: `Activated ability ${ref.ability + 1}` }
  const full = worded.cost === '' ? worded.effect : `${worded.cost}: ${worded.effect}`
  if (worded.unworded === true) return { label: worded.effect }
  if (worded.keyword !== undefined) return { label: worded.keyword, detail: full }
  return { label: full }
}

/**
 * The cards of `seat` with an ability to activate: each ability in `options.abilities` whose card the seat may use it
 * from. One the card's words mark as a mana ability is left out, and a spectator (no seat) has none.
 */
export function abilityPlays(
  options: PriorityOptions,
  objectsById: Readonly<Record<number, ObjectView>>,
  seat: PlayerId | null,
): Activations {
  if (seat === null || options.abilities === undefined || options.abilities.length === 0) return NONE
  const found = new Map<ObjectId, Play[]>()
  for (const ref of options.abilities) {
    const object = objectsById[ref.object]
    if (object === undefined || !usable(object, seat)) continue
    const worded = object.activated?.find(
      (entry) => entry.ability !== undefined && sameAbility(entry.ability, ref),
    )
    if (worded?.mana === true) continue
    const { label, detail } = wording(ref, worded)
    const play: Play = {
      key: abilityKey(ref),
      label,
      words: `Activate ${spoken(label)}`,
      action: { Activate: ref },
      ...(detail === undefined ? {} : { detail }),
    }
    const list = found.get(ref.object)
    if (list === undefined) found.set(ref.object, [play])
    else list.push(play)
  }
  return found
}

/** What a card with abilities is called to a screen reader: "Activate {T}: Draw a card", or the count when several. */
export function activateWords(plays: readonly Play[]): string {
  const [only] = plays
  return plays.length === 1 && only !== undefined ? only.words : `Activate one of ${plays.length} abilities`
}

/**
 * The note of every card with an ability to activate: the words a screen reader adds to its name, and the abilities
 * (`abilityKey`) the preview of the card marks as available now.
 */
export function activationNotes(activations: Activations): CardNotes {
  const notes = new Map<ObjectId, CardNote>()
  for (const [id, plays] of activations) {
    notes.set(id, { words: activateWords(plays), usable: plays.map((play) => play.key) })
  }
  return notes
}

const lowerFirst = (text: string): string => text.charAt(0).toLowerCase() + text.slice(1)

/**
 * The hand's notes (why a card cannot be played) with the activation notes laid over them. A hand card that cannot be
 * cast but has an ability to activate (cycling) is legal, so it keeps no hint on its foot; its preview and its spoken
 * name say what it can do and why it cannot be cast.
 */
export function mergeNotes(hand: CardNotes, activation: CardNotes): CardNotes {
  if (activation.size === 0) return hand
  const merged = new Map(hand)
  for (const [id, note] of activation) {
    const why = hand.get(id)
    merged.set(
      id,
      why === undefined
        ? note
        : {
            ...note,
            words: `${note.words}, not castable: ${why.words}`,
            ...(why.line === undefined ? {} : { line: `Not castable: ${lowerFirst(why.line)}` }),
          },
    )
  }
  return merged
}

/** The line of one mana ability in a chooser: the mana it adds, told apart by place on the card when two add the same. */
function manaPlay(option: ManaOption, all: readonly ManaOption[]): Play {
  const gives = { ...givesOfOptions([option]), cost: [], sacrifice: false }
  const colour = madeWords(gives)
  const twins = all.filter((other) => other.produces === option.produces).length > 1
  const label = twins ? `${colour} (ability ${option.source.ability + 1})` : colour
  return {
    key: `mana-${option.source.ability}`,
    label: `Add ${label} mana`,
    words: `Add ${label} mana`,
    action: { ActivateMana: option.source },
  }
}

/**
 * Everything a click on card `id` could do, in the order of the chooser: play the land or cast the spell, activate an
 * ability, tap for mana. A land wins over a spell when the card is offered as both, as a click always played it.
 */
export function playsOf(
  id: ObjectId,
  name: string,
  options: PriorityOptions,
  abilities: Activations,
  mana: readonly ManaOption[],
): readonly Play[] {
  const first: Play[] = options.playable_lands.includes(id)
    ? [{ key: 'land', label: `Play ${name}`, words: `Play ${name}`, action: { PlayLand: id } }]
    : options.castable.includes(id)
      ? [{ key: 'cast', label: `Cast ${name}`, words: `Cast ${name}`, action: { CastSpell: id } }]
      : []
  return [...first, ...(abilities.get(id) ?? []), ...mana.map((option) => manaPlay(option, mana))]
}
