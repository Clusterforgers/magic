/** The target prompt in plain words: what is asked, how many, and who or what each legal target is. */
import type { ObjectView, PlayerId, Target } from '@/protocol/generated'
import { powerToughness } from '../cardModel'
import { countOf } from './combatText'

/** A key that is equal for equal targets, for maps and React keys. */
export function targetKey(target: Target): string {
  return 'Player' in target
    ? `player:${target.Player}`
    : `object:${target.Object.id}:${target.Object.counter}`
}

/** Whether two targets are the same player or the same object. */
export function sameTarget(a: Target, b: Target): boolean {
  return targetKey(a) === targetKey(b)
}

/** The targets of `legal` once each, in the order they first appear. */
export function distinctTargets(legal: readonly Target[]): Target[] {
  const seen = new Map<string, Target>()
  for (const target of legal) if (!seen.has(targetKey(target))) seen.set(targetKey(target), target)
  return [...seen.values()]
}

/** How many targets are asked for: "1 target", "2 targets", "up to 2 targets", "1 to 3 targets". */
export function boundsWords(min: number, max: number): string {
  if (min === max) return countOf(max, 'target')
  if (min === 0) return `up to ${countOf(max, 'target')}`
  return `${min} to ${countOf(max, 'target')}`
}

/** What a prompt knows when it speaks about the targets chosen so far. */
export interface TargetsAsked {
  /** Who asks: "Lightning Bolt" or "Mulldrifter's ability". */
  subject: string
  /** The specification in the words of a card ("target creature you control"); empty when the engine gave none. */
  wording: string
  min: number
  max: number
  /** How many targets are chosen so far. */
  chosen: number
}

/** The live sentence of the prompt: the question at first, then what is chosen and what is left to do. */
export function targetsSentence({ subject, wording, min, max, chosen }: TargetsAsked): string {
  if (chosen === 0) return `${subject}: choose ${wording === '' ? boundsWords(min, max) : wording}.`
  if (chosen < min)
    return `${subject}: ${chosen} of ${countOf(min, 'target')} chosen. Choose ${min - chosen} more.`
  const head = `${subject}: ${countOf(chosen, 'target')} chosen.`
  if (max === 1) return `${head} Confirm the target, or choose another.`
  if (chosen >= max) return `${head} Confirm the targets, or take one back.`
  return `${head} Confirm the targets, or choose up to ${max - chosen} more.`
}

/** "Mulldrifter's", and "Grizzly Bears'" for a name that ends in an s. */
export function possessive(name: string): string {
  return name.endsWith('s') ? `${name}'` : `${name}'s`
}

/** Who is asking, as the sentence names it: the spell by its name, a triggered ability as its source's. */
export function subjectWords(name: string | undefined, triggered: boolean): string {
  if (name === undefined) return triggered ? 'A triggered ability' : 'Your spell'
  return triggered ? `${possessive(name)} ability` : name
}

/** The name of a seat, as it is given to a button that picks it. */
export type SeatNamer = (seat: PlayerId) => string

/** Whose an object is or where it is: "Bo" on the battlefield, "on the stack, Bo", "in Bo's graveyard". */
function placeWords(object: ObjectView, seatName: SeatNamer): string {
  const { zone } = object
  if (zone === 'Battlefield') return seatName(object.controller)
  if (zone === 'Stack') return `on the stack, ${seatName(object.controller)}`
  if (zone === 'Exile') return 'in exile'
  if (zone === 'PhasedOut') return `phased out, ${seatName(object.controller)}`
  if ('Graveyard' in zone) return `in ${seatName(zone.Graveyard)}'s graveyard`
  if ('Hand' in zone) return `in ${seatName(zone.Hand)}'s hand`
  if ('Library' in zone) return `in ${seatName(zone.Library)}'s library`
  return `in ${seatName(zone.Command)}'s command zone`
}

/**
 * An object as a button names it: its name (`name`, which the caller may have numbered), its power and toughness, and
 * in brackets whose it is or where it is. An ability on the stack is named after its source.
 */
export function objectWords(object: ObjectView, name: string, seatName: SeatNamer): string {
  const stats = powerToughness(object)
  const base = `${object.kind === 'Ability' ? `${possessive(name)} ability` : name}${stats === null ? '' : ` ${stats}`}`
  return `${base} (${placeWords(object, seatName)})`
}
