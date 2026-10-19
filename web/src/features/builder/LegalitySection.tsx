import { ScaleIcon, TriangleAlertIcon } from 'lucide-react'
import { useId } from 'react'
import { formatLabel } from '@/features/decks/format'
import type { Issue } from '@/protocol/generated'
import { legalityLines } from './report'

interface LegalitySectionProps {
  /** `report.legality`: why the deck is not legal in its format. */
  issues: readonly Issue[]
  /** The format the report checked the deck in. */
  format: string
  /** The draft differs from the saved deck, so the issues are those of the saved one. */
  stale: boolean
}

/**
 * The ways the deck breaks the rules of its format, as plain sentences. They are legality, not support, so they sit
 * apart from the cards the game cannot play by itself. Nothing is drawn for a legal deck.
 */
export function LegalitySection({ issues, format, stale }: LegalitySectionProps) {
  const id = useId()
  if (issues.length === 0) return null
  return (
    <section
      aria-labelledby={`${id}-legality`}
      data-slot="legality"
      className="grid gap-2 rounded-lg border border-destructive bg-destructive/12 p-3"
    >
      <h3
        id={`${id}-legality`}
        className="flex flex-wrap items-center gap-x-2 text-sm font-semibold text-destructive"
      >
        <ScaleIcon aria-hidden="true" className="size-4" />
        Not legal in {formatLabel(format)}
        <span className="font-normal">
          {issues.length === 1 ? '1 issue' : `${issues.length} issues`}
          {stale ? ', as saved' : ''}
        </span>
      </h3>
      <ul aria-label="Legality issues" className="grid gap-1 text-sm">
        {legalityLines(issues).map((line) => (
          <li key={line.key} className="flex items-start gap-2">
            <span className="flex h-5 shrink-0 items-center text-destructive">
              <TriangleAlertIcon aria-hidden="true" className="size-4" />
            </span>
            {line.text}
          </li>
        ))}
      </ul>
    </section>
  )
}
