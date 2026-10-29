import { useCallback, useEffect, useState } from 'react'
import { PreconError } from '@/net/precons'
import type { PreconsApi } from '@/net/precons'
import type { PreconListItem } from '@/protocol/generated'

/** The precons the chooser offers, as the lobby sees them. */
export type PreconChoices =
  | { status: 'loading' }
  | { status: 'ready'; precons: PreconListItem[] }
  | { status: 'failed'; message: string }

type Settled = Exclude<PreconChoices, { status: 'loading' }>

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

/** Loads `GET /api/precons` once, and again when `reload` is called. */
export function usePreconChoices(api: Pick<PreconsApi, 'list'>): {
  state: PreconChoices
  reload: () => void
} {
  const [round, setRound] = useState(0)
  // Tagged with the round it answers, so a new round reads as loading without resetting state inside the effect.
  const [settled, setSettled] = useState<{ round: number; state: Settled } | null>(null)
  const reload = useCallback(() => setRound((n) => n + 1), [])

  useEffect(() => {
    let stale = false
    api.list().then(
      (list) => {
        if (!stale) setSettled({ round, state: { status: 'ready', precons: list.precons } })
      },
      (error: unknown) => {
        if (!stale) setSettled({ round, state: { status: 'failed', message: preconListWords(error) } })
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
