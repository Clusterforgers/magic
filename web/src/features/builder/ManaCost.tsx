import { Fragment } from 'react'
import { ManaPips } from '@/features/table/ManaPips'
import { cn } from '@/lib/utils'
import { faceCosts } from './manaText'

/** The mana cost of a card as symbols, one group per face; nothing for a card without a cost. */
export function ManaCost({ cost, className }: { cost: string; className?: string }) {
  const faces = faceCosts(cost)
  if (faces.length === 0) return null
  return (
    <span data-slot="mana-cost" className={cn('flex shrink-0 items-center gap-1', className)}>
      {faces.map((pips, index) => (
        <Fragment key={`${index}-${pips.map((pip) => pip.symbol).join('')}`}>
          {index > 0 ? (
            <span aria-hidden="true" className="text-xs text-muted-foreground">
              //
            </span>
          ) : null}
          <ManaPips
            pips={pips}
            sizeClass="size-4 text-label"
            label={faces.length > 1 ? `Mana cost of face ${index + 1}` : 'Mana cost'}
          />
        </Fragment>
      ))}
    </span>
  )
}
