/** What an ability belongs to when no object gives it (the Ring): its name in the log and on the stack, and its words. Pure. */
import type { AbilitySource, Granter } from '@/protocol/generated'

/** What the clause of a Ring level's ability does, as the log says it (CR 701.54c, levels 2 to 4). */
const RING_CLAUSE: Readonly<Record<number, string>> = {
  2: 'draw a card, then discard a card',
  3: "that creature's controller sacrifices it at end of combat",
  4: 'each opponent loses 3 life',
}

/** What the origin of an ability says gives it, when that is not an object: the Ring's level. */
export function granterOfOrigin(origin: AbilitySource | undefined): Granter | undefined {
  if (origin === undefined || typeof origin === 'string' || !('Granted' in origin)) return undefined
  const { Granted: grant } = origin
  return 'Ring' in grant ? { Ring: { level: grant.Ring.level } } : undefined
}

/** The name the ability goes by, and the stack row says: "The Ring". */
export function granterName(_granter: Granter): string {
  return 'The Ring'
}

/** "The Ring's ability", or "the Ring's ability" inside a sentence. */
export function granterAbility(granter: Granter, start: boolean, delayed = false): string {
  const name = granterName(granter)
  const named = start ? name : `${name.charAt(0).toLowerCase()}${name.slice(1)}`
  return `${named}'s ${delayed ? 'delayed ' : ''}ability`
}

/** Who has the ability and whose it is: "Nazgûl, Bo's Ring-bearer" (a hidden source has no name to give). */
export function granterHolder(_granter: Granter, source: string, controller: string): string {
  return `${source === '' ? 'a face-down permanent' : source}, ${controller}'s Ring-bearer`
}

/** What the ability does, in the words of the log; undefined when the level gives none. */
export function granterClause(granter: Granter): string | undefined {
  return RING_CLAUSE[granter.Ring.level]
}
