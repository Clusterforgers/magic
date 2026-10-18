import { useCallback, useEffect, useState } from 'react'
import type { Api, DeckInfo } from '@/net/api'
import { describeApiError } from './apiErrors'

/** The deck list as the page sees it. */
export type DecksState =
  { status: 'loading' } | { status: 'ready'; decks: DeckInfo[] } | { status: 'failed'; message: string }

type Settled = Exclude<DecksState, { status: 'loading' }>

/** Loads `GET /api/decks` once, and again when `reload` is called. */
export function useDecks(api: Api): { state: DecksState; reload: () => void } {
  const [round, setRound] = useState(0)
  // Tagged with the round it answers, so a new round reads as loading without resetting state inside the effect.
  const [settled, setSettled] = useState<{ round: number; state: Settled } | null>(null)
  const reload = useCallback(() => setRound((n) => n + 1), [])

  useEffect(() => {
    let stale = false
    api.listDecks().then(
      (decks) => {
        if (!stale) setSettled({ round, state: { status: 'ready', decks } })
      },
      (error: unknown) => {
        if (!stale) setSettled({ round, state: { status: 'failed', message: describeApiError(error) } })
      },
    )
    return () => {
      stale = true
    }
  }, [api, round])

  return {
    state: settled !== null && settled.round === round ? settled.state : { status: 'loading' },
    reload,
  }
}
