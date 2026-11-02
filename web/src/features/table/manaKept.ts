/** The mana of a pool that outlasts the step (`ManaPool.kept`), by kind, with its words. */
import type { ManaPool, ManaType } from '@/protocol/generated'

// The order of `ManaPool.counts` and `Kept` on the wire.
const TYPES: readonly ManaType[] = ['White', 'Blue', 'Black', 'Red', 'Green', 'Colorless']

/** How much of a kind stays past the step, and until when in words: "1 kept until end of turn". */
export interface KeptMana {
  count: number
  words: string
}

/** The mana of `type` the pool keeps past the step, or null when all of it empties as the step ends. */
export function keptOf(pool: ManaPool, type: ManaType): KeptMana | null {
  const at = TYPES.indexOf(type)
  const turn = pool.kept?.turn_end[at] ?? 0
  const combat = pool.kept?.combat_end[at] ?? 0
  if (turn + combat === 0) return null
  const parts = [
    ...(turn > 0 ? [`${turn} until end of turn`] : []),
    ...(combat > 0 ? [`${combat} until end of combat`] : []),
  ]
  return { count: turn + combat, words: `kept: ${parts.join(', ')}` }
}
