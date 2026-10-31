import { LegalitySection } from '@/features/builder/LegalitySection'
import type { DeckReport } from '@/protocol/generated'
import { DeckStateBadge } from './DeckStateBadge'
import { BLOCKED_MEANING, countsText } from './format'

/**
 * What the check says of a list before it is saved: the state and how many cards the game runs whole or leaves to the
 * players, as the builder's bar words it, and the rules of the format the list breaks. With no report (the check did not
 * answer) it says the check happens at saving.
 */
export function ImportCheck({ report }: { report: DeckReport | null }) {
  if (report === null) {
    return (
      <p data-slot="import-check" className="text-sm text-muted-foreground">
        The deck is checked when it is saved.
      </p>
    )
  }
  const { state, counts } = report.summary
  return (
    <div data-slot="import-check" data-state={state} className="grid gap-2">
      <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
        <DeckStateBadge state={state} />
        <span>{countsText(counts)}</span>
      </p>
      <LegalitySection issues={report.legality} format={report.format} stale={false} />
      {state === 'blocked' ? (
        <p className="text-sm text-muted-foreground">You can still save it. {BLOCKED_MEANING}.</p>
      ) : null}
    </div>
  )
}
