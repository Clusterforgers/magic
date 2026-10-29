import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'
import type { Support } from '@/protocol/generated'
import { supportEdge } from '../SupportMark'
import type { ClauseLine } from './model'

interface ClauseListProps {
  lines: readonly ClauseLine[]
  support: Support
  /** The card's text is still being read, so a quote may yet come. */
  pending: boolean
}

/**
 * The clauses of a card the game leaves to the table: the quoted words of the card beside the plain reason (stacked
 * under `sm`). A clause whose words are not known shows the reason alone, or a placeholder while they are read.
 */
export function ClauseList({ lines, support, pending }: ClauseListProps) {
  return (
    <ul className="grid gap-2" data-slot="clauses">
      {lines.map((line) => {
        const waiting = line.quote === null && pending
        return (
          <li
            key={line.key}
            data-clause=""
            className={cn(
              'grid gap-1 sm:gap-4',
              line.quote !== null || waiting ? 'sm:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]' : '',
            )}
          >
            {line.quote !== null ? (
              <blockquote
                className={cn(
                  'min-w-0 border-l-2 pl-3 text-sm break-words text-foreground',
                  supportEdge(support),
                )}
              >
                <span className="sr-only">Text: </span>
                {`“${line.quote}”`}
              </blockquote>
            ) : waiting ? (
              <Skeleton className="h-4 w-3/4 self-center" />
            ) : null}
            <p className="min-w-0 text-sm break-words text-muted-foreground">
              <span className="sr-only">Reason: </span>
              {line.text}
            </p>
          </li>
        )
      })}
    </ul>
  )
}
