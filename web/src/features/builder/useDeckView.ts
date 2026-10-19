import { useCallback, useEffect, useState } from 'react'
import { describeDeckError, isDecksUnavailable, isSessionEnded } from '@/features/decks/errors'
import { DeckError } from '@/net/decks'
import type { DecksApi } from '@/net/decks'
import type { DeckView } from '@/protocol/generated'

/** The stored deck as the builder page sees it while loading. */
export type DeckViewState =
  | { status: 'loading' }
  | { status: 'ready'; view: DeckView }
  | { status: 'missing' }
  | { status: 'unavailable' }
  | { status: 'failed'; message: string; sessionEnded: boolean }

type Settled = Exclude<DeckViewState, { status: 'loading' }>

function failed(error: unknown): Settled {
  if (isDecksUnavailable(error)) return { status: 'unavailable' }
  if (error instanceof DeckError && error.code === 'not_found') return { status: 'missing' }
  return { status: 'failed', message: describeDeckError(error), sessionEnded: isSessionEnded(error) }
}

/** Loads `GET /api/decks/{id}` once, and again when `reload` is called or the id changes. */
export function useDeckView(api: DecksApi, id: string): { state: DeckViewState; reload: () => void } {
  const [round, setRound] = useState(0)
  const key = `${id}#${round}`
  // Tagged with the request it answers, so a new request reads as loading without resetting state in the effect.
  const [settled, setSettled] = useState<{ key: string; state: Settled } | null>(null)
  const reload = useCallback(() => setRound((n) => n + 1), [])

  useEffect(() => {
    let stale = false
    api.get(id).then(
      (view) => {
        if (!stale) setSettled({ key, state: { status: 'ready', view } })
      },
      (error: unknown) => {
        if (!stale) setSettled({ key, state: failed(error) })
      },
    )
    return () => {
      stale = true
    }
  }, [api, id, key])

  return { state: settled?.key === key ? settled.state : { status: 'loading' }, reload }
}
