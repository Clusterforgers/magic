import { useQuery } from '@tanstack/react-query'
import { useCallback } from 'react'
import type { DecksApi } from '@/net/decks'
import { deckListKey, listCache } from '@/net/listCache'
import type { DeckListItem } from '@/protocol/generated'
import { describeDeckError, isDecksUnavailable, isSessionEnded } from './errors'

/** The deck list as the page sees it. */
export type DeckListState =
  | { status: 'loading' }
  | { status: 'ready'; decks: DeckListItem[]; refreshing: boolean }
  | { status: 'unavailable' }
  | { status: 'failed'; message: string; sessionEnded: boolean }

function failed(error: unknown): DeckListState {
  if (isDecksUnavailable(error)) return { status: 'unavailable' }
  return { status: 'failed', message: describeDeckError(error), sessionEnded: isSessionEnded(error) }
}

/**
 * Reads `GET /api/decks` on every mount and when `reload` is called, through the list cache (`listCache`): a list seen
 * before shows at once, marked as refreshing, while the new one is read, so neither a reopened chooser nor a reload
 * flashes; a server without decks is its own state, not a failure.
 */
export function useDeckList(api: Pick<DecksApi, 'list'>): { state: DeckListState; reload: () => void } {
  const query = useQuery({ queryKey: deckListKey(api), queryFn: () => api.list() }, listCache)
  const { refetch } = query
  const reload = useCallback(() => void refetch(), [refetch])

  let state: DeckListState = { status: 'loading' }
  if (query.isFetching && query.isError) {
    state =
      query.data === undefined
        ? { status: 'loading' }
        : { status: 'ready', decks: query.data, refreshing: true }
  } else if (query.isError) state = failed(query.error)
  else if (query.data !== undefined)
    state = { status: 'ready', decks: query.data, refreshing: query.isFetching }
  return { state, reload }
}
