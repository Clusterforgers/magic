import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { Card } from './Card'
import type { CardSize } from './cardSize'
import { GhostCard } from './fx/GhostCard'
import { withGhosts } from './fx/ghosts'
import { NO_GHOSTS } from './fx/model'
import type { Ghost } from './fx/model'
import { useRoving } from './useRoving'

interface CardRowProps {
  ids: readonly ObjectId[]
  /** Permanents that just died, drawn in their old places while their effect plays; decoration, never in `ids`. */
  ghosts?: readonly Ghost[]
  /** Names the list for a screen reader, such as "Alice's creatures". */
  label: string
  size: CardSize
  /** One line that scrolls sideways instead of wrapping onto more lines. */
  single?: boolean
  className?: string
}

/** A list of cards with one tab stop: Tab enters it, the arrow keys, Home and End move along it. */
export function CardRow({ ids, ghosts = NO_GHOSTS, label, size, single = false, className }: CardRowProps) {
  const roving = useRoving(ids)
  if (ids.length === 0 && ghosts.length === 0) return null
  const items = ghosts.length === 0 ? ids : withGhosts(ids, ghosts)
  return (
    <ul
      role="list"
      aria-label={label}
      // A row holding only ghosts has nothing to announce.
      aria-hidden={ids.length === 0 || undefined}
      onFocus={roving.onFocus}
      onKeyDown={roving.onKeyDown}
      className={cn('flex gap-1.5 p-1', single ? 'overflow-x-auto' : 'flex-wrap', className)}
    >
      {items.map((item) =>
        typeof item === 'number' ? (
          <li key={item} className="flex">
            <Card id={item} size={size} tabbable={item === roving.current} />
          </li>
        ) : (
          <GhostCard key={`ghost-${item.token}`} ghost={item} size={size} />
        ),
      )}
    </ul>
  )
}
