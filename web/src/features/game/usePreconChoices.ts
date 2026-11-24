import { useQuery } from '@tanstack/react-query'
import { useCallback } from 'react'
import { listCache, preconListKey } from '@/net/listCache'
import { PreconError } from '@/net/precons'
import type { PreconsApi } from '@/net/precons'
import type { PreconListItem } from '@/protocol/generated'

/** The precons the chooser offers, as the lobby sees them. */
// Also read by the sources that list the precons.
export type PreconChoices =
  | { status: 'loading' }
  | { status: 'ready'; precons: PreconListItem[] }
  | { status: 'failed'; message: string }

/** What to tell a player about a precon list that did not load. */
export function preconListWords(error: unknown): string {
  if (error instanceof PreconError && error.code === 'rate_limited') {
    return 'The server is answering too many requests from this address. Wait a moment, then try again.'
  }
  if (error instanceof PreconError && error.code === 'cards_unavailable') {
    return 'This server has no card data, so it has no precons to offer.'
  }
  return 'The precons could not be loaded. Check the connection, then try again.'
}

/**
 * Reads `GET /api/precons` once `enabled` (the Precons tab opened, or a favourite needs it), on every mount after that
 * and when `reload` is called, through the list cache (`listCache`): precons seen before show at once while they are
 * read again.
 */
export function usePreconChoices(
  api: Pick<PreconsApi, 'list'>,
  enabled = true,
): {
  state: PreconChoices
  reload: () => void
} {
  const query = useQuery({ queryKey: preconListKey(api), queryFn: () => api.list(), enabled }, listCache)
  const { refetch } = query
  const reload = useCallback(() => void refetch(), [refetch])

  let state: PreconChoices = { status: 'loading' }
  if (query.isError && !query.isFetching) state = { status: 'failed', message: preconListWords(query.error) }
  else if (query.data !== undefined) state = { status: 'ready', precons: query.data.precons }
  return { state, reload }
}
