/**
 * A cost that takes a target of its own spell or ability (CR 601.2c, 601.2h): sacrificing the creature Goblin
 * Bombardment targets is legal, but the ability then has no target. The engine marks the clash on both questions,
 * whichever comes second (`CardsOptions.targeted`, `TargetOptions.paid`); these words warn before the answer is sent,
 * and never refuse it.
 */
import type { ObjectId, Zone } from '@/protocol/generated'

/** The words on a card the cost would take that is a target: in the cost pick, and in the target pick. */
export const TARGETED_BADGE = 'Targeted'
export const PAYS_COST_BADGE = 'Pays the cost'

/** What the warning needs of an object: its name and where it is. */
export interface ClashObject {
  name: string
  zone?: Zone | undefined
}

/** The warning shown before a clashing answer is sent. */
export interface ClashWarning {
  /** The objects that are both a target and taken by the cost. */
  ids: readonly ObjectId[]
  /** The sentence: what leaves and what it means for the spell or ability. */
  text: string
}

/** The ids of `chosen` that are also in `clashing`, in the order chosen. */
export function clashes(chosen: readonly ObjectId[], clashing: readonly ObjectId[] | undefined): ObjectId[] {
  if (clashing === undefined || clashing.length === 0) return []
  const set = new Set(clashing)
  return chosen.filter((id) => set.has(id))
}

/** "the battlefield", "your graveyard": the zone an object leaves, in a few words. */
export function leftZoneWords(zone: Zone | undefined): string {
  if (zone === undefined || zone === 'Battlefield' || zone === 'PhasedOut') return 'the battlefield'
  if (zone === 'Stack') return 'the stack'
  if (zone === 'Exile') return 'exile'
  if ('Graveyard' in zone) return 'the graveyard'
  if ('Hand' in zone) return 'the hand'
  if ('Library' in zone) return 'the library'
  return 'the command zone'
}

/**
 * The warning for an answer whose `chosen` objects include `clashing` ones, or null when none clash. `all` says
 * whether every target the answer knows is lost, so the spell or ability is left with no target at all.
 */
export function clashWarning({
  chosen,
  clashing,
  source,
  objects,
  all,
}: {
  chosen: readonly ObjectId[]
  clashing: readonly ObjectId[] | undefined
  source: string | undefined
  objects: (id: ObjectId) => ClashObject | undefined
  all: boolean
}): ClashWarning | null {
  const ids = clashes(chosen, clashing)
  if (ids.length === 0) return null
  const asker = source ?? 'The spell or ability'
  const first = objects(ids[0] as ObjectId)
  const zone = leftZoneWords(first?.zone)
  const what =
    ids.length === 1
      ? `${asker}'s target leaves ${zone}`
      : `${ids.length} of ${asker}'s targets leave ${zone}`
  const names = ids.map((id) => objects(id)?.name ?? 'a card')
  const paid = ids.length === 1 ? `${names[0]} pays the cost` : `${names.join(' and ')} pay the cost`
  const end = all
    ? 'It will have no target.'
    : ids.length === 1
      ? 'It will lose that target.'
      : 'It will lose those targets.'
  return { ids, text: `${what}: ${paid}. ${end}` }
}
