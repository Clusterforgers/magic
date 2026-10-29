/** How a card says which of its clauses the game leaves to the table: the list of them and the mark on the face. */
import { Hand } from 'lucide-react'
import type { MouseEvent } from 'react'
import { cn } from '@/lib/utils'
import type { ManualClause, ObjectView } from '@/protocol/generated'
import { clausesOf, showClauses } from './clauses'
import { useMenuStore } from './menuStore'

/** The clauses and the reason the game leaves each to the table. */
export function ByHandList({
  clauses,
  className,
  id,
}: {
  clauses: readonly ManualClause[]
  className?: string
  id?: string
}) {
  return (
    <ul id={id} className={cn('grid gap-1', className)}>
      {clauses.map((clause) => (
        <li key={clause.text}>
          <q>{clause.text}</q> <span className="text-muted-foreground">{clause.reason}</span>
        </li>
      ))}
    </ul>
  )
}

/** Where a mark sits: over the picture, at the head of its corner, or as a chip among the text face's. */
export type ByHandSpot = 'corner' | 'chip'

interface ByHandMarkProps {
  object: ObjectView
  spot: ByHandSpot
  /** The icon alone: a small face has no room for the words. */
  compact: boolean
  /** The text size of the face (`SIZE[size].text`). */
  text: string
}

/**
 * The mark of a card with clauses left to the table. A press on it (a mouse or a finger) opens the list, and the
 * focused card opens the same list with the `h` key (`useManualMenu`), so the mark itself is not in the tab order: the
 * card is the one stop and says so in its spoken name. Outside a table it only shows, with the clauses as its tooltip.
 */
export function ByHandMark({ object, spot, compact, text }: ByHandMarkProps) {
  const store = useMenuStore()
  const clauses = clausesOf(object)
  if (clauses.length === 0) return null
  const open = (event: MouseEvent<HTMLElement>): void => {
    if (store === null) return
    // The press is the mark's, not the card's: it neither pins the preview nor answers a prompt.
    event.stopPropagation()
    event.preventDefault()
    const mark = event.currentTarget
    showClauses(store, object.id, mark, mark.closest<HTMLElement>('[data-object-id]') ?? mark)
  }
  return (
    <span
      data-slot="card-support"
      data-spot={spot}
      title={`Played by hand: ${clauses.map((clause) => `${clause.text} (${clause.reason})`).join('; ')}`}
      aria-hidden="true"
      onClick={store === null ? undefined : open}
      // A finger on the mark is not the long press of the card's menu.
      onPointerDown={store === null ? undefined : (event) => event.stopPropagation()}
      className={cn(
        'inline-flex h-6 min-w-6 items-center justify-center gap-1 rounded-full bg-support-manual font-semibold whitespace-nowrap text-background shadow-sm',
        spot === 'corner' ? 'pointer-events-auto border border-background px-1' : 'px-2',
        store === null ? null : 'cursor-pointer hover:brightness-110',
        text,
      )}
    >
      <Hand aria-hidden="true" className="size-3 shrink-0" />
      {compact ? null : <span>By hand</span>}
    </span>
  )
}
