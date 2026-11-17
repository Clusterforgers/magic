/** A relic's rarity word as a small pill in its tier's colour. */
import { cn } from '@/lib/utils'
import type { Rarity } from '@/protocol/generated'
import { RARITY_TONE } from './rarity'

export function RarityTag({ rarity, className }: { rarity: Rarity; className?: string }) {
  const tone = RARITY_TONE[rarity]
  return (
    <span
      data-rarity={rarity}
      className={cn(
        'inline-flex shrink-0 items-center rounded-full border bg-surface px-2 text-xs font-semibold',
        tone.text,
        tone.border,
        className,
      )}
    >
      {rarity}
    </span>
  )
}
