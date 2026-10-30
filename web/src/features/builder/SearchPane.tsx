import {
  ChevronLeftIcon,
  ChevronRightIcon,
  CircleAlertIcon,
  SearchIcon,
  SearchXIcon,
  TriangleAlertIcon,
} from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'
import type { CardsApi } from '@/net/cards'
import type { FixMarksApi } from '@/net/fixMarks'
import type { CardHit, CardSearchPage, DeckZone } from '@/protocol/generated'
import type { Draft } from './deckEdit'
import { FilterBar } from './FilterBar'
import { ResultRow } from './ResultRow'
import { NO_FILTERS } from './search'
import type { SearchFilters } from './search'
import { useCardSearch } from './useCardSearch'

interface SearchPaneProps {
  api: CardsApi
  /** The deck being built, for the copies each hit shows and the zones its menu offers. */
  draft: Draft
  onAdd: (card: CardHit, zone: DeckZone) => void
  /** Opens the preview of a card from its name; `opener` is the control to return focus to. */
  onPreview: (card: CardHit, opener: HTMLElement) => void
  /** The identity of the deck's commanders, or null while it has none: the filters then offer to stay within it and a hit outside it says so. */
  commanderIdentity?: readonly string[] | null
  /** A search the builder asks for (a slot looking for a partner): applied, and the box focused, each time `id` changes. */
  seed?: { id: number; filters: SearchFilters } | null
  /** The fix-mark endpoints; with them a hit that is not Verified gets the flag. */
  fixMarks?: FixMarksApi
  /** Sets the display of the pane (`flex` or `hidden`), which the page switches under the M breakpoint. */
  className?: string
}

// The shape of the results while they load: rows like the ones that will come.
function RowsSkeleton() {
  return (
    <div className="grid gap-1">
      {[0, 1, 2, 3, 4, 5].map((row) => (
        <Skeleton key={row} className="h-11 rounded-lg" />
      ))}
    </div>
  )
}

function Pager({ page, onGo, busy }: { page: CardSearchPage; onGo: (page: number) => void; busy: boolean }) {
  const perPage = Math.max(1, page.per_page)
  const pages = Math.max(1, Math.ceil(page.total / perPage))
  const from = page.cards.length === 0 ? 0 : (page.page - 1) * perPage + 1
  const to = from === 0 ? 0 : from + page.cards.length - 1
  const first = page.page <= 1 || busy
  const last = page.page >= pages || busy
  return (
    <div className="flex items-center justify-between gap-2 border-t pt-2">
      <p className="text-sm text-muted-foreground tabular-nums">
        {page.total === 0 ? 'No cards' : `${from}–${to} of ${page.total}`}
      </p>
      <div className="flex items-center gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-disabled={first || undefined}
          onClick={() => !first && onGo(page.page - 1)}
        >
          <ChevronLeftIcon aria-hidden="true" />
          Previous
        </Button>
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-disabled={last || undefined}
          onClick={() => !last && onGo(page.page + 1)}
        >
          Next
          <ChevronRightIcon aria-hidden="true" />
        </Button>
      </div>
    </div>
  )
}

/**
 * The search half of the builder: a search box, the filters and the hits, 40 to a page. Typing is waited out for a
 * moment before a search is sent; Enter in the box adds the first hit to the main deck (or searches at once when the
 * hits are for older text). With nothing typed or chosen it says how to start instead of listing every card.
 */
export function SearchPane({
  api,
  draft,
  onAdd,
  onPreview,
  commanderIdentity = null,
  seed = null,
  fixMarks,
  className,
}: SearchPaneProps) {
  const [filters, setFilters] = useState<SearchFilters>(NO_FILTERS)
  const [seeded, setSeeded] = useState(seed?.id ?? 0)
  const box = useRef<HTMLInputElement>(null)
  if (seed !== null && seed.id !== seeded) {
    setSeeded(seed.id)
    setFilters(seed.filters)
  }
  const seedId = seed?.id ?? 0
  useEffect(() => {
    if (seedId > 0) box.current?.focus()
  }, [seedId])
  const search = useCardSearch(api, filters)
  const hintId = useId()
  const listRef = useRef<HTMLDivElement>(null)
  const { state } = search
  const shown = state.status === 'ready' ? state.page : state.status === 'loading' ? state.previous : null
  const busy = state.status === 'loading' || search.pending

  const go = (page: number): void => {
    search.goTo(page)
    if (listRef.current !== null) listRef.current.scrollTop = 0
  }

  let body: ReactNode
  if (state.status === 'idle') {
    body = (
      <EmptyState icon={SearchIcon} as="h3" title="Find cards to add">
        Type a name or some rules text, or narrow by color identity, type, mana value or format. Enter adds
        the first card to the main deck.
      </EmptyState>
    )
  } else if (state.status === 'unavailable') {
    body = (
      <Alert variant="warning" role="status">
        <TriangleAlertIcon aria-hidden="true" />
        <AlertTitle>Card search is not available</AlertTitle>
        <AlertDescription>
          This server has no card database, so cards cannot be searched or added. The cards already in the
          deck can still be changed and saved.
        </AlertDescription>
      </Alert>
    )
  } else if (state.status === 'failed') {
    body = (
      <Alert variant="destructive">
        <CircleAlertIcon aria-hidden="true" />
        <AlertTitle>Could not search the cards</AlertTitle>
        <AlertDescription>{state.message}</AlertDescription>
        <div className="mt-2">
          <Button type="button" onClick={search.retry}>
            Retry
          </Button>
        </div>
      </Alert>
    )
  } else if (shown === null) {
    body = (
      <Loading label="Searching cards…">
        <RowsSkeleton />
      </Loading>
    )
  } else if (shown.cards.length === 0) {
    body = (
      <EmptyState
        icon={SearchXIcon}
        as="h3"
        title="No cards match"
        action={
          <Button type="button" variant="outline" onClick={() => setFilters(NO_FILTERS)}>
            Clear search
          </Button>
        }
      >
        Check the spelling, or take a filter off.
      </EmptyState>
    )
  } else {
    body = (
      <ul
        aria-label="Search results"
        aria-busy={busy}
        className={cn('grid gap-1 transition-opacity duration-fast ease-out', busy && 'opacity-60')}
      >
        {shown.cards.map((card) => (
          <ResultRow
            key={card.oracle_id}
            card={card}
            draft={draft}
            identity={commanderIdentity}
            onAdd={onAdd}
            onPreview={onPreview}
            fixMarks={fixMarks}
          />
        ))}
      </ul>
    )
  }

  return (
    <section aria-label="Card search" className={cn('surface min-h-0 flex-col gap-3 p-3', className)}>
      <div className="relative">
        <SearchIcon
          aria-hidden="true"
          className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
        />
        <Input
          ref={box}
          type="search"
          aria-label="Search cards"
          aria-describedby={hintId}
          autoComplete="off"
          spellCheck={false}
          enterKeyHint="search"
          maxLength={100}
          placeholder="Search by name or rules text"
          value={filters.text}
          className="pl-8"
          onChange={(event) => setFilters({ ...filters, text: event.target.value })}
          onKeyDown={(event) => {
            if (event.key !== 'Enter' || event.nativeEvent.isComposing) return
            event.preventDefault()
            if (search.pending) {
              search.flush()
              return
            }
            const first = state.status === 'ready' ? state.page.cards[0] : undefined
            if (first === undefined) return
            onAdd(first, 'main')
            event.currentTarget.select()
          }}
        />
        <p id={hintId} className="sr-only">
          Enter adds the first card to the main deck.
        </p>
      </div>
      <FilterBar filters={filters} onChange={setFilters} commanderIdentity={commanderIdentity} />
      <p role="status" className="sr-only">
        {state.status === 'loading' ? 'Searching cards…' : null}
        {state.status === 'ready' ? `${state.page.total} cards found.` : null}
      </p>
      <div ref={listRef} className="relative min-h-0 flex-1 sm:overflow-y-auto sm:[scrollbar-gutter:stable]">
        {body}
      </div>
      {shown === null || shown.cards.length === 0 ? null : <Pager page={shown} onGo={go} busy={busy} />}
    </section>
  )
}
