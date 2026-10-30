import { LoaderCircleIcon, TriangleAlertIcon } from 'lucide-react'
import { useId } from 'react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { Switch } from '@/components/ui/switch'
import { countsText } from '@/features/decks/format'
import { cn } from '@/lib/utils'
import type { Counts } from '@/protocol/generated'
import type { SortKey } from './report'
import type { CheckStatus } from './useLiveCheck'

interface CheckBarProps {
  /** Copies by support state, from the report's summary. */
  counts: Counts
  /** The counts are those of the saved deck while the draft's cards differ: no check of the draft has come back. */
  asSaved: boolean
  /** Whether the counts are of the draft on screen, a newer check is on its way, or the check failed. */
  status: CheckStatus
  /** What went wrong, in words, while `status` is `failed`. */
  error: string | null
  /** Where the counts come from, for the note of a failed check. */
  source: 'saved' | 'draft'
  onRetry: () => void
  issuesOnly: boolean
  onIssuesOnly: (on: boolean) => void
  sort: SortKey
  onSort: (sort: SortKey) => void
  /** Controls that sit before the switch and the sort, such as the choice of how cards are drawn. */
  leading?: ReactNode
}

/**
 * The deck check under the name: how many cards the game plays by itself ("96 of 100 verified, 4 played by hand"),
 * what is happening to those counts while the draft is checked, the Issues only switch and the sort that puts the cards
 * with issues first. The status line is always in the page, so a screen reader hears "Checking…" and then the new counts.
 */
export function CheckBar({
  counts,
  asSaved,
  status,
  error,
  source,
  onRetry,
  issuesOnly,
  onIssuesOnly,
  sort,
  onSort,
  leading,
}: CheckBarProps) {
  const id = useId()
  const checking = status === 'checking'
  const checked = status === 'current' && source === 'draft'
  return (
    <div data-slot="deck-check" className="flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
      <div className="flex min-w-0 flex-wrap items-baseline gap-x-2">
        <p data-slot="check-counts" className="text-sm font-medium tabular-nums">
          {countsText(counts)}
          {asSaved ? <span className="font-normal text-muted-foreground"> · as saved</span> : null}
        </p>
        <p
          data-slot="check-status"
          role="status"
          className={cn('flex items-center gap-1 text-xs text-muted-foreground', !checking && 'sr-only')}
        >
          {checking ? (
            <>
              <LoaderCircleIcon aria-hidden="true" className="size-3 motion-safe:animate-spin" />
              Checking…
            </>
          ) : checked ? (
            `Checked: ${countsText(counts)}.`
          ) : null}
        </p>
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        {leading}
        <div className="flex h-8 items-center gap-2 pointer-coarse:h-11">
          <Switch
            id={`${id}-only`}
            aria-labelledby={`${id}-only-label`}
            checked={issuesOnly}
            onCheckedChange={onIssuesOnly}
          />
          <Label id={`${id}-only-label`} htmlFor={`${id}-only`}>
            Issues only
          </Label>
        </div>
        <NativeSelect
          aria-label="Sort cards"
          size="sm"
          value={sort}
          className="w-36"
          onChange={(event) => onSort(event.target.value === 'issues' ? 'issues' : 'name')}
        >
          <NativeSelectOption value="name">By name</NativeSelectOption>
          <NativeSelectOption value="issues">Issues first</NativeSelectOption>
        </NativeSelect>
      </div>
      {status === 'failed' ? (
        <div data-slot="check-failed" role="status" className="grid basis-full gap-1 text-sm">
          <p className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <TriangleAlertIcon aria-hidden="true" className="size-4 shrink-0 text-warning" />
            <span>
              Could not check the changes. The counts are those of{' '}
              {source === 'saved' ? 'the saved deck' : 'the last check'}.
            </span>
            <Button type="button" variant="outline" size="xs" onClick={onRetry}>
              Try again
            </Button>
          </p>
          {error === null ? null : <p className="text-xs text-muted-foreground">{error}</p>}
        </div>
      ) : null}
    </div>
  )
}
