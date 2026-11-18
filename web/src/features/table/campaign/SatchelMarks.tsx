/**
 * A boss's Satchel as a list: each card's name with its mark, "returns · +{N}" for a reusable card (N the generic mana
 * its next cast costs more) or "once". The table's Satchel list and the fight's intro show the same rows.
 */
import { RotateCcwIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { BossSatchelView } from '@/protocol/generated'
import { satchelMark, satchelMarkTitle } from './reworkedBoss'

/** The boss's Satchel cards with their marks. */
export function SatchelMarks({
  cards,
  className,
}: {
  cards: readonly BossSatchelView[]
  className?: string
}) {
  return (
    <ul aria-label="Satchel cards" data-slot="satchel-marks" className={cn('grid gap-1 text-sm', className)}>
      {cards.map((card, at) => (
        <li
          key={`${at}-${card.name}`}
          data-slot="satchel-mark"
          data-reusable={card.reusable || undefined}
          className="flex min-w-0 items-center justify-between gap-2 rounded-md border bg-surface px-2 py-1"
        >
          <span className="min-w-0 truncate font-medium">{card.name}</span>
          <span
            title={satchelMarkTitle(card)}
            className={cn(
              'inline-flex shrink-0 items-center gap-1 rounded-full px-1.5 text-xs leading-5 font-semibold tabular-nums',
              card.reusable ? 'bg-boss/15 text-boss' : 'text-muted-foreground',
            )}
          >
            {card.reusable ? <RotateCcwIcon aria-hidden="true" className="size-3" /> : null}
            {satchelMark(card)}
            <span className="sr-only">. {satchelMarkTitle(card)}</span>
          </span>
        </li>
      ))}
    </ul>
  )
}
