/** The targets of a stack item as words: the line on its chip, the text a screen reader hears and the panel's rows. */
import type { ObjectId, PlayerId } from '@/protocol/generated'
import { joinWords, targetName } from './eventText'
import { live, MAX_ARROWS, MAX_NAMED } from './stackTargets'
import type { StackEntry } from './stackTargets'

/** How targets are named; the log's `EventNames` fits. */
export interface TargetNamer {
  seat(seat: PlayerId): string
  card(id: ObjectId): string
}

/** What a chip with nothing left to aim at says. */
export const NO_LEGAL_TARGET = 'no legal target'

/** The names a chip shows, at most `MAX_NAMED` of the targets still legal, and how many more there are. */
export function namedTargets(entry: StackEntry, n: TargetNamer): { names: string[]; more: number } {
  const targets = live(entry)
  return {
    names: targets.slice(0, MAX_NAMED).map((t) => targetName(t.target, n)),
    more: targets.length - MAX_NAMED,
  }
}

/** The arrows an item does not get because of `MAX_ARROWS`. */
export const arrowsLeftOut = (entry: StackEntry): number => Math.max(0, live(entry).length - MAX_ARROWS)

/** "Hill Giant, Bo +2 more", the marker for an item whose targets are all gone, and nothing for an item with none. */
export function targetsLine(entry: StackEntry, n: TargetNamer): string {
  if (entry.targets.length === 0) return ''
  const { names, more } = namedTargets(entry, n)
  if (names.length === 0) return NO_LEGAL_TARGET
  return `${names.join(', ')}${more > 0 ? ` +${more} more` : ''}`
}

/** "Lightning Bolt → Hill Giant": the item's name and its targets in one short line. */
export function chipLine(name: string, entry: StackEntry, n: TargetNamer): string {
  const targets = targetsLine(entry, n)
  return targets === '' ? name : `${name} → ${targets}`
}

/** What a screen reader hears after the chip's label: ", targeting Hill Giant and Bo", or that nothing is left to aim at. */
export function targetsAria(entry: StackEntry, n: TargetNamer): string {
  if (entry.targets.length === 0) return ''
  const targets = live(entry)
  if (targets.length === 0) return `, ${NO_LEGAL_TARGET} left`
  const names = targets.map((t) => targetName(t.target, n))
  const shown =
    names.length > MAX_NAMED ? [...names.slice(0, MAX_NAMED), `${names.length - MAX_NAMED} more`] : names
  const gone = entry.targets.length - targets.length
  return `, targeting ${joinWords(shown)}${gone > 0 ? `, ${gone} no longer legal` : ''}`
}
