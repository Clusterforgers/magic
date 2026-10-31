import type { ManaPool as ManaPoolValue } from '@/protocol/generated'
import { ManaPool } from './ManaPool'
import { useRiderWords } from './useRiderWords'

/** The mana pool of a seat of the game, its riders worded from the permanents that made the mana. */
export function LiveManaPool({ pool, className }: { pool: ManaPoolValue; className?: string }) {
  return (
    <ManaPool pool={pool} words={useRiderWords(pool)} {...(className === undefined ? {} : { className })} />
  )
}
