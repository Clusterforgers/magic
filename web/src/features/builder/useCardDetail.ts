import { useCallback, useEffect, useState } from 'react'
import { isCardsUnavailable } from '@/net/cards'
import type { CardsApi } from '@/net/cards'
import type { CardDetail } from '@/protocol/generated'
import { describeCardsError } from './errors'

/** The card detail as the preview sees it. */
export type DetailState =
  | { status: 'loading' }
  | { status: 'ready'; detail: CardDetail }
  | { status: 'unavailable' }
  | { status: 'failed'; message: string }

type Settled = Exclude<DetailState, { status: 'loading' }>

/** What `useCardDetail` gives the preview. */
export interface CardDetailLoad {
  state: DetailState
  /** Asks for the card again after a failure. */
  retry: () => void
}

/**
 * Reads `GET /api/cards/{oracle_id}` for the card asked for and cancels the request when another card is asked for or
 * the caller goes away. With `null` nothing is asked.
 */
export function useCardDetail(api: CardsApi, oracleId: string | null): CardDetailLoad {
  const [round, setRound] = useState(0)
  const retry = useCallback(() => setRound((n) => n + 1), [])
  const key = `${oracleId ?? ''}#${round}`
  const [settled, setSettled] = useState<{ key: string; state: Settled } | null>(null)

  useEffect(() => {
    if (oracleId === null) return
    const controller = new AbortController()
    let stale = false
    api.get(oracleId, controller.signal).then(
      (detail) => {
        if (!stale) setSettled({ key, state: { status: 'ready', detail } })
      },
      (error: unknown) => {
        if (stale) return
        const state: Settled = isCardsUnavailable(error)
          ? { status: 'unavailable' }
          : { status: 'failed', message: describeCardsError(error) }
        setSettled({ key, state })
      },
    )
    return () => {
      stale = true
      controller.abort()
    }
  }, [api, oracleId, key])

  const state: DetailState = settled !== null && settled.key === key ? settled.state : { status: 'loading' }
  return { state, retry }
}
