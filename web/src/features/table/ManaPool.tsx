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

/** The mana a seat has floating, by kind; "empty" when there is none. */
export function ManaPool({ pool, className }: { pool: ManaPoolValue; className?: string }) {
  const held = TYPES.flatMap((type, index) => {
    const count = pool.counts[index] ?? 0
    return count > 0 ? [{ type, count }] : []
  })
  return (
    <div
      role="group"
      aria-label="Mana pool"
      className={cn('flex flex-wrap items-center gap-x-2 gap-y-1 text-sm', className)}
    >
      <span className="text-xs text-muted-foreground">Mana</span>
      {held.length === 0 ? (
        <span className="text-xs text-muted-foreground">empty</span>
      ) : (
        held.map(({ type, count }) => (
          <span key={type} className="inline-flex items-center gap-1">
            <span
              aria-hidden="true"
              className={cn(
                'inline-flex size-4 items-center justify-center rounded-full text-[0.6rem] font-bold ring-1 ring-foreground/25',
                PIP[TONE[type]],
              )}
            >
              {manaLetter(type)}
            </span>
            <span className="font-semibold tabular-nums" aria-hidden="true">
              {count}
            </span>
            <span className="sr-only">
              {count} {type.toLowerCase()}
            </span>
          </span>
        ))
      )}
    </div>
  )
}
