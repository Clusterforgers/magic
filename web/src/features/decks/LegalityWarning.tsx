import { TriangleAlertIcon } from 'lucide-react'
import { useId } from 'react'
import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'
import type { Issue } from '@/protocol/generated'
import { notLegalText, STILL_PLAYS } from './legality'

interface LegalityWarningProps {
  /** The format the deck was judged in. */
  format: string
  /** The issues, or only their count where the page has no more. */
  issues: readonly Pick<Issue, 'code'>[] | number
  /** The sentences listed under the title, where there is room; none for a one-line warning. */
  lines?: readonly { key: string; text: string }[]
  /** The `data-slot` the warning carries, for pages that look for it. */
  slot?: string
  /** Words after the title, such as " (as saved)". */
  aside?: ReactNode
  className?: string
}

/**
 * A deck that breaks its format's rules, as a warning: an icon and "Not legal in Commander: 2 banned cards", then that it
 * can still be played and, where there is room, each issue. Never colour alone, never a refusal.
 */
export function LegalityWarning({ format, issues, lines, aside, slot, className }: LegalityWarningProps) {
  const id = useId()
  return (
    <section
      aria-labelledby={id}
      data-slot={slot ?? 'legality-warning'}
      className={cn('grid gap-1 rounded-lg border border-warning bg-warning/12 px-3 py-2 text-sm', className)}
    >
      <p id={id} className="flex items-start gap-2 font-semibold text-warning">
        <TriangleAlertIcon aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
        <span className="min-w-0">
          {notLegalText(format, issues)}
          {aside}
        </span>
      </p>
      <p className="pl-6 text-foreground">{STILL_PLAYS}</p>
      {lines === undefined || lines.length === 0 ? null : (
        <ul aria-label="Legality issues" className="grid list-disc gap-1 pl-10 text-foreground">
          {lines.map((line) => (
            <li key={line.key}>{line.text}</li>
          ))}
        </ul>
      )}
    </section>
  )
}
