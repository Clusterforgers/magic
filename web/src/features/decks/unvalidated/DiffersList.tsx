import { TriangleAlertIcon } from 'lucide-react'
import type { ReadDiffers } from '@/protocol/generated'
import { DIFFERS_ENGINE, DIFFERS_PRINTED, DIFFERS_TITLE } from './model'

interface DiffersListProps {
  differs: readonly ReadDiffers[]
}

/**
 * The clauses a card's draft words differently from the card, each a highlighted row with the printed clause beside the
 * engine's own English (stacked under `sm`), so a reader sees exactly what differs before marking the draft correct.
 */
export function DiffersList({ differs }: DiffersListProps) {
  if (differs.length === 0) return null
  return (
    <section data-slot="differs" aria-label={DIFFERS_TITLE} className="grid gap-2">
      <p className="flex items-start gap-1 text-sm font-medium text-foreground">
        <TriangleAlertIcon aria-hidden="true" className="mt-0.5 size-4 shrink-0 text-support-partial" />
        {DIFFERS_TITLE}
      </p>
      <ul className="grid gap-2">
        {differs.map((row) => (
          <li
            key={`${row.face}:${row.span.start}`}
            data-differs=""
            className="grid gap-2 rounded-md border border-l-4 border-support-partial bg-support-partial/10 p-2 sm:grid-cols-2 sm:gap-4"
          >
            <div className="min-w-0">
              <p className="text-xs text-muted-foreground">{DIFFERS_PRINTED}</p>
              <blockquote className="text-sm break-words text-foreground">{`“${row.printed}”`}</blockquote>
            </div>
            <div className="min-w-0">
              <p className="text-xs text-muted-foreground">{DIFFERS_ENGINE}</p>
              <blockquote className="text-sm break-words text-foreground">{`“${row.written}”`}</blockquote>
            </div>
          </li>
        ))}
      </ul>
    </section>
  )
}
