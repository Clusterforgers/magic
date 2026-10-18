import { cn } from '@/lib/utils'
import type { ObjectId } from '@/protocol/generated'
import { Card } from './Card'
import type { CardSize } from './cardSize'
import { useRoving } from './useRoving'

interface CardRowProps {
  ids: readonly ObjectId[]
  /** Names the list for a screen reader, such as "Alice's creatures". */
  label: string
  size: CardSize
  /** One line that scrolls sideways instead of wrapping onto more lines. */
  single?: boolean
  className?: string
}

/** A list of cards with one tab stop: Tab enters it, the arrow keys, Home and End move along it. */
export function CardRow({ ids, label, size, single = false, className }: CardRowProps) {
  const roving = useRoving(ids)
  if (ids.length === 0) return null
  return (
    <ul
      role="list"
      aria-label={label}
      onFocus={roving.onFocus}
      onKeyDown={roving.onKeyDown}
      className={cn('flex gap-1.5 p-1', single ? 'overflow-x-auto' : 'flex-wrap', className)}
    >
      {ids.map((id) => (
        <li key={id} className="flex">
          <Card id={id} size={size} tabbable={id === roving.current} />
        </li>
      ))}
    </ul>
  )
}
