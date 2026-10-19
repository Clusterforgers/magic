import { cn } from '@/lib/utils'
import type { ManaPool as ManaPoolValue, ManaType } from '@/protocol/generated'
import { manaLetter } from './cardModel'
import type { PipTone } from './cardModel'
import { PIP } from './cardSize'

// The order of `ManaPool.counts` on the wire.
const TYPES: readonly ManaType[] = ['White', 'Blue', 'Black', 'Red', 'Green', 'Colorless']
const TONE: Record<ManaType, PipTone> = {
  White: 'white',
  Blue: 'blue',
  Black: 'black',
  Red: 'red',
  Green: 'green',
  Colorless: 'colorless',
}

/** One mana symbol, a coloured dot with the kind's letter; decoration, so name the mana in words beside it. */
export function ManaDot({ type }: { type: ManaType }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        'inline-flex size-4 items-center justify-center rounded-full text-[0.6rem] font-bold ring-1 ring-foreground/25',
        PIP[TONE[type]],
      )}
    >
      {manaLetter(type)}
    </span>
  )
}

/** The kinds of mana a pool holds, with their counts, in the order of the wire. */
export function manaHeld(pool: ManaPoolValue): Array<{ type: ManaType; count: number }> {
  return TYPES.flatMap((type, index) => {
    const count = pool.counts[index] ?? 0
    return count > 0 ? [{ type, count }] : []
  })
}

/** The mana a seat has floating, by kind, each in words for a screen reader; an empty pool draws nothing. */
export function ManaPool({ pool, className }: { pool: ManaPoolValue; className?: string }) {
  const held = manaHeld(pool)
  if (held.length === 0) return null
  return (
    <div
      role="group"
      aria-label="Mana pool"
      className={cn('flex flex-wrap items-center gap-x-2 gap-y-1 text-sm', className)}
    >
      {held.map(({ type, count }) => (
        <span key={type} className="inline-flex items-center gap-1">
          <ManaDot type={type} />
          <span className="font-semibold tabular-nums" aria-hidden="true">
            {count}
          </span>
          <span className="sr-only">
            {count} {type.toLowerCase()}
          </span>
        </span>
      ))}
    </div>
  )
}
