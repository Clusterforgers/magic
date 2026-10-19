import { useCallback, useEffect, useState } from 'react'
import type { DecksApi } from '@/net/decks'
import type { DeckListItem } from '@/protocol/generated'
import { describeDeckError, isDecksUnavailable, isSessionEnded } from './errors'

/** The deck list as the page sees it. */
export type DeckListState =
  | { status: 'loading' }
  | { status: 'ready'; decks: DeckListItem[]; refreshing: boolean }
  | { status: 'unavailable' }
  | { status: 'failed'; message: string; sessionEnded: boolean }

type Settled = Exclude<DeckListState, { status: 'loading' }>

function failed(error: unknown): Settled {
  if (isDecksUnavailable(error)) return { status: 'unavailable' }
  return { status: 'failed', message: describeDeckError(error), sessionEnded: isSessionEnded(error) }
}

/**
 * Loads `GET /api/decks` once, and again when `reload` is called. A reload keeps the rows it has, marked as refreshing,
 * so the list does not flash; a server without decks is its own state, not a failure.
 */
export function useDeckList(api: DecksApi): { state: DeckListState; reload: () => void } {
  const [round, setRound] = useState(0)
  // Tagged with the round it answers, so a new round reads as loading without resetting state inside the effect.
  const [settled, setSettled] = useState<{ round: number; state: Settled } | null>(null)
  const reload = useCallback(() => setRound((n) => n + 1), [])

  useEffect(() => {
    let stale = false
    api.list().then(
      (decks) => {
        if (!stale) setSettled({ round, state: { status: 'ready', decks, refreshing: false } })
      },
      (error: unknown) => {
        if (!stale) setSettled({ round, state: failed(error) })
      },
    )
    return () => {
      stale = true
    }
  }, [api, round])

  let state: DeckListState = { status: 'loading' }
  if (settled !== null) {
    if (settled.round === round) state = settled.state
    else if (settled.state.status === 'ready') state = { ...settled.state, refreshing: true }
  }
  return { state, reload }
}
