import { Lock, Sparkles } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ManaPool as ManaPoolValue, ManaType } from '@/protocol/generated'
import { manaLetter } from './cardModel'
import type { PipTone } from './cardModel'
import { PIP } from './cardSize'
import { limits, manaRiders, riderKey, riderWords } from './manaRider'

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
        'inline-flex size-4 items-center justify-center rounded-full text-xs font-bold ring-1 ring-foreground/25',
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

/**
 * The mana of a pool that may be spent only on some things (Mishra's Workshop's), by kind in the order of the wire. A
 * unit whose uses are riders alone limits nothing (`manaRiders`).
 */
export function manaRestricted(pool: ManaPoolValue): Array<{ type: ManaType; count: number }> {
  const units = (pool.restricted ?? []).filter(limits)
  return TYPES.flatMap((type) => {
    const count = units.filter((entry) => entry.mana === type).length
    return count > 0 ? [{ type, count }] : []
  })
}

/**
 * The mana a seat has floating, by kind: the symbol and `×2` for the amount, each in words for a screen reader. Mana
 * with a rider (it sets something off when it pays for a spell, and pays for anything) follows the plain mana with a
 * small mark and the rider's words; mana that may be spent only on some things comes last with a lock on it. An empty
 * pool draws nothing. `words` gives the sentence of each rider by `riderKey`; the kind of spell it waits for stands in
 * for a rider it has none for.
 */
export function ManaPool({
  pool,
  className,
  words,
}: {
  pool: ManaPoolValue
  className?: string
  words?: ReadonlyMap<string, string>
}) {
  const held = manaHeld(pool)
  const riders = manaRiders(pool)
  const restricted = manaRestricted(pool)
  if (held.length === 0 && riders.length === 0 && restricted.length === 0) return null
  return (
    <div
      role="group"
      aria-label="Mana pool"
      className={cn('flex flex-wrap items-center gap-x-2 gap-y-1 text-sm', className)}
    >
      {held.map(({ type, count }) => (
        <span key={type} data-mana={type} className="inline-flex items-center gap-0.5">
          <ManaDot type={type} />
          <span className="font-semibold tabular-nums" aria-hidden="true">
            ×{count}
          </span>
          <span className="sr-only">
            {count} {type.toLowerCase()}
          </span>
        </span>
      ))}
      {riders.map(({ type, count, riders: carried }) => {
        const said = carried
          .map((rider) => words?.get(riderKey(rider)) ?? riderWords(rider, undefined))
          .join(' ')
        return (
          <span
            key={`rider-${type}`}
            data-mana={type}
            data-rider="true"
            title={said}
            className="inline-flex items-center gap-0.5"
          >
            <ManaDot type={type} />
            <span className="font-semibold tabular-nums" aria-hidden="true">
              ×{count}
            </span>
            <Sparkles aria-hidden="true" className="size-3 text-muted-foreground" />
            <span className="sr-only">
              {count} {type.toLowerCase()}, with a rider: {said}
            </span>
          </span>
        )
      })}
      {restricted.map(({ type, count }) => (
        <span
          key={`restricted-${type}`}
          data-mana={type}
          data-restricted="true"
          title="Restricted mana: it can be spent only on what its source allows"
          className="inline-flex items-center gap-0.5 rounded-full border border-dashed border-foreground/40 px-1"
        >
          <ManaDot type={type} />
          <Lock aria-hidden="true" className="size-3 text-muted-foreground" />
          <span className="font-semibold tabular-nums" aria-hidden="true">
            ×{count}
          </span>
          <span className="sr-only">
            {count} {type.toLowerCase()}, restricted
          </span>
        </span>
      ))}
    </div>
  )
}
