import { useId } from 'react'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { Switch } from '@/components/ui/switch'
import { countsText } from '@/features/decks/format'
import type { Counts } from '@/protocol/generated'
import type { SortKey } from './report'

interface CheckBarProps {
  /** Copies by support state, from the report's summary. */
  counts: Counts
  /** The draft differs from the saved deck, so the counts are those of the saved one. */
  stale: boolean
  issuesOnly: boolean
  onIssuesOnly: (on: boolean) => void
  sort: SortKey
  onSort: (sort: SortKey) => void
}

/**
 * The deck check under the name: how many cards the game plays by itself ("96 of 100 verified, 3 manual, 1 partial"),
 * the Issues only switch and the sort that puts the cards with issues first.
 */
export function CheckBar({ counts, stale, issuesOnly, onIssuesOnly, sort, onSort }: CheckBarProps) {
  const id = useId()
  return (
    <div data-slot="deck-check" className="flex flex-wrap items-center justify-between gap-x-3 gap-y-2">
      <p data-slot="check-counts" className="text-sm font-medium tabular-nums">
        {countsText(counts)}
        {stale ? <span className="font-normal text-muted-foreground"> · as saved</span> : null}
      </p>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
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
    </div>
  )
}
