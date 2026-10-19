import { useCallback, useEffect, useMemo, useState } from 'react'
import { isCardsUnavailable } from '@/net/cards'
import type { CardsApi } from '@/net/cards'
import type { CardSearchPage } from '@/protocol/generated'
import { describeCardsError } from './errors'
import { queryOf } from './search'
import type { SearchFilters } from './search'

/** How long typing must pause before a search is sent, in milliseconds. */
export const SEARCH_PAUSE_MS = 250

/** The search as the pane sees it. */
export type SearchState =
  | { status: 'idle' }
  | { status: 'loading'; previous: CardSearchPage | null }
  | { status: 'ready'; page: CardSearchPage }
  | { status: 'unavailable' }
  | { status: 'failed'; message: string }

type Settled = Exclude<SearchState, { status: 'idle' } | { status: 'loading' }>

/** What `useCardSearch` gives the pane. */
export interface CardSearch {
  state: SearchState
  /** The page asked for, counted from 1. */
  page: number
  /** True while the filters differ from the ones searched for, that is, while typing is still being waited out. */
  pending: boolean
  goTo: (page: number) => void
  /** Searches for the filters now instead of waiting out the pause. */
  flush: () => void
  retry: () => void
}

/**
 * Runs `GET /api/cards` for `filters` once typing has paused, one request at a time: a newer search cancels the one
 * before it. A change of filters starts again at page 1, and a reload keeps the rows it has while the next page loads.
 * Pass a `filters` object that changes identity only when a value does. With nothing to search for the state is idle and
 * no request is made.
 */
export function useCardSearch(
  api: CardsApi,
  filters: SearchFilters,
  pauseMs: number = SEARCH_PAUSE_MS,
): CardSearch {
  const [applied, setApplied] = useState(filters)
  // Compared by content: `applied` changes identity only when a value does, so filters that come back equal ask nothing.
  const filtersKey = JSON.stringify(filters)
  const appliedKey = JSON.stringify(applied)
  const pending = filtersKey !== appliedKey
  useEffect(() => {
    if (!pending) return
    const timer = window.setTimeout(() => setApplied(filters), pauseMs)
    return () => window.clearTimeout(timer)
  }, [filters, pending, pauseMs])
  const flush = useCallback(() => {
    if (pending) setApplied(filters)
  }, [filters, pending])

  // The page belongs to the filters it was chosen for; other filters read as page 1 without resetting state in an effect.
  const [pager, setPager] = useState({ key: appliedKey, page: 1 })
  const page = pager.key === appliedKey ? pager.page : 1
  const goTo = useCallback(
    (next: number) => setPager({ key: appliedKey, page: Math.max(1, Math.floor(next)) }),
    [appliedKey],
  )

  const [round, setRound] = useState(0)
  const retry = useCallback(() => setRound((n) => n + 1), [])
  const query = useMemo(() => queryOf(applied, page), [applied, page])
  const requestKey = `${JSON.stringify(query)}#${round}`
  const [settled, setSettled] = useState<{ key: string; state: Settled } | null>(null)

  useEffect(() => {
    if (query === null) return
    const controller = new AbortController()
    let stale = false
    api.search(query, controller.signal).then(
      (found) => {
        if (!stale) setSettled({ key: requestKey, state: { status: 'ready', page: found } })
      },
      (error: unknown) => {
        if (stale) return
        const state: Settled = isCardsUnavailable(error)
          ? { status: 'unavailable' }
          : { status: 'failed', message: describeCardsError(error) }
        setSettled({ key: requestKey, state })
      },
    )
    return () => {
      stale = true
      controller.abort()
    }
  }, [api, query, requestKey])

  let state: SearchState = { status: 'idle' }
  if (query !== null) {
    if (settled !== null && settled.key === requestKey) state = settled.state
    else
      state = { status: 'loading', previous: settled?.state.status === 'ready' ? settled.state.page : null }
  }
  return { state, page, pending, goTo, flush, retry }
}
