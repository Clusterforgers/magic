import { SearchIcon } from 'lucide-react'
import { useId, useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import type { MissingCardView, MissingState } from '@/protocol/generated'
import { LOOKS } from './looks'
import { count, filterMissing, stateCounts } from './model'

/** How many rows show at first and with each "Show more". */
export const PAGE_ROWS = 50

const FILTERS: ReadonlyArray<MissingState | 'all'> = ['all', 'partial', 'manual', 'waiting', 'other']

/**
 * What is missing: every card that does not play automatically, the most played first, with its state and what holds
 * it back first in plain words. A search over name and blocker and a state filter narrow it; rows come in pages.
 */
export function MissingList({ cards }: { cards: readonly MissingCardView[] }) {
  const [query, setQuery] = useState('')
  const [state, setState] = useState<MissingState | 'all'>('all')
  const [rows, setRows] = useState(PAGE_ROWS)
  const searchId = useId()
  const counts = useMemo(() => stateCounts(cards), [cards])
  const found = useMemo(() => filterMissing(cards, { query, state }), [cards, query, state])
  const shown = found.slice(0, rows)

  return (
    <section aria-labelledby="missing-title" className="surface grid content-start gap-4 p-4 sm:p-6">
      <div className="grid gap-1">
        <h2 id="missing-title" className="text-lg font-semibold tracking-tight">
          What we are missing
        </h2>
        <p className="text-sm text-muted-foreground">
          {`${count(cards.length)} cards play partly or wholly by hand, the most played first.`}
        </p>
      </div>
      <div className="grid gap-3">
        <label htmlFor={searchId} className="sr-only">
          Search the missing cards
        </label>
        <div className="relative">
          <SearchIcon
            aria-hidden="true"
            className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            id={searchId}
            type="search"
            value={query}
            placeholder="Card name or blocker"
            className="pl-9"
            onChange={(event) => {
              setQuery(event.target.value)
              setRows(PAGE_ROWS)
            }}
          />
        </div>
        <div role="group" aria-label="State" className="flex flex-wrap gap-2">
          {FILTERS.map((filter) => {
            const number = filter === 'all' ? cards.length : counts[filter]
            const word = filter === 'all' ? 'All' : LOOKS[filter].word
            return (
              <Button
                key={filter}
                type="button"
                size="sm"
                variant={state === filter ? 'default' : 'outline'}
                aria-pressed={state === filter}
                onClick={() => {
                  setState(filter)
                  setRows(PAGE_ROWS)
                }}
              >
                {word}
                <span className="tabular-nums opacity-80">{count(number)}</span>
              </Button>
            )
          })}
        </div>
      </div>
      <p role="status" className="text-sm text-muted-foreground empty:hidden">
        {found.length === cards.length
          ? null
          : `${count(found.length)} of ${count(cards.length)} cards match.`}
      </p>
      {found.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {cards.length === 0
            ? 'Every card plays automatically.'
            : 'No card matches. Try other words or All.'}
        </p>
      ) : (
        <ul className="divide-y divide-border" data-slot="missing">
          {shown.map((card) => {
            const look = LOOKS[card.state]
            return (
              <li
                key={card.oracle_id}
                data-state={card.state}
                className="grid grid-cols-[3.5rem_minmax(0,1fr)] gap-x-3 gap-y-1 py-2 sm:grid-cols-[3.5rem_minmax(0,14rem)_minmax(0,1fr)]"
              >
                <span className="text-sm text-muted-foreground tabular-nums" title="EDHREC rank">
                  {card.rank === null ? '–' : `#${count(card.rank)}`}
                </span>
                <span className="flex min-w-0 items-center gap-2">
                  <span
                    role="img"
                    aria-label={look.word}
                    title={`${look.word}: ${look.meaning}`}
                    className={cn(
                      'inline-grid size-6 shrink-0 place-items-center rounded-full border',
                      look.mark,
                    )}
                  >
                    <look.Icon aria-hidden="true" className="size-3.5" />
                  </span>
                  <span className="truncate text-sm font-medium" title={card.name}>
                    {card.name}
                  </span>
                </span>
                <span
                  className="col-start-2 text-sm text-muted-foreground break-words sm:col-start-3"
                  title={card.detail === '' ? undefined : card.detail}
                >
                  {card.blocker}
                  {card.blockers > 1 ? (
                    <span className="text-xs">{` · +${card.blockers - 1} more`}</span>
                  ) : null}
                </span>
              </li>
            )
          })}
        </ul>
      )}
      {found.length > rows ? (
        <Button
          type="button"
          variant="outline"
          className="justify-self-start"
          onClick={() => setRows(rows + PAGE_ROWS)}
        >
          {`Show ${count(Math.min(PAGE_ROWS, found.length - rows))} more`}
        </Button>
      ) : null}
    </section>
  )
}
