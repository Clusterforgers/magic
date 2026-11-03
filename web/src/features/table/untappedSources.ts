/** The untapped mana sources of a seat counted by the colour each can make, for the figures beside the pool. */
import type { ManaType, ObjectView, PlayerId } from '@/protocol/generated'
import { COLOURS, givesNow } from './manaMade'

/** The kinds in the order of `ManaPool.counts` on the wire. */
export const SOURCE_KINDS: readonly ManaType[] = [...COLOURS, 'Colorless']

/**
 * How many untapped permanents of a seat can make each kind, in `SOURCE_KINDS` order. A source that can make two
 * colours counts for both; one that makes any colour counts for the five colours.
 */
export function sourceCounts(objects: Iterable<ObjectView | undefined>, seat: PlayerId): number[] {
  const counts = SOURCE_KINDS.map(() => 0)
  for (const object of objects) {
    if (object === undefined) continue
    // The seat's own sources are counted the same for every viewer, so the seat stands in for the viewer.
    const gives = givesNow(object, seat)
    if (gives === null) continue
    const kinds = new Set<ManaType>(gives.any ? COLOURS : [])
    for (const way of gives.ways) for (const made of way.made) kinds.add(made)
    SOURCE_KINDS.forEach((kind, index) => {
      if (kinds.has(kind)) counts[index] = (counts[index] ?? 0) + 1
    })
  }
  return counts
}
