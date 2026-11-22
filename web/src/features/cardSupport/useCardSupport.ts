import { useCallback, useEffect, useRef, useState } from 'react'
import type { CardSupportApi } from '@/net/cardSupport'
import type { CardSupport, MissingCards } from '@/protocol/generated'

/** How often the page asks whether a new snapshot or check exists. */
export const POLL_MS = 60_000

/** What the page shows: the overview and the missing cards, each loading, ready or failed. */
export interface CardSupportState {
  /** The overview; null until the first answer. */
  overview: CardSupport | null
  /** The missing cards of the overview's snapshot; null until loaded. */
  missing: MissingCards | null
  /** The message of the last failure; null while the last call worked. */
  error: string | null
  /** When the overview last arrived (ms since the epoch); 0 before. */
  fetchedAt: number
  /** The snapshot id that arrived after the first, so the page can say it changed; null otherwise. */
  updated: number | null
  /** Asks again now. */
  retry: () => void
}

const message = (error: unknown): string => (error instanceof Error ? error.message : 'Unknown error.')

/**
 * Loads the overview, then the missing cards, and asks again every `POLL_MS` while the page is visible (and at once when
 * it becomes visible); the missing cards load again only when the snapshot id changes. A failed poll keeps what is shown.
 */
export function useCardSupport(api: CardSupportApi, pollMs = POLL_MS): CardSupportState {
  const [overview, setOverview] = useState<CardSupport | null>(null)
  const [missing, setMissing] = useState<MissingCards | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [fetchedAt, setFetchedAt] = useState(0)
  const [updated, setUpdated] = useState<number | null>(null)
  const [round, setRound] = useState(0)
  const shownId = useRef<number | null | undefined>(undefined)

  const retry = useCallback(() => setRound((n) => n + 1), [])

  useEffect(() => {
    let live = true
    void (async () => {
      try {
        const next = await api.overview()
        if (!live) return
        setOverview(next)
        setFetchedAt(Date.now())
        setError(null)
        const id = next.snapshot?.id ?? null
        if (shownId.current === id) return
        const list = id === null ? { snapshot_id: null, cards: [] } : await api.missing()
        if (!live) return
        if (shownId.current !== undefined && id !== null) setUpdated(id)
        shownId.current = id
        setMissing(list)
      } catch (failure) {
        if (live) setError(message(failure))
      }
    })()
    return () => {
      live = false
    }
  }, [api, round])

  useEffect(() => {
    const visible = () => typeof document === 'undefined' || document.visibilityState !== 'hidden'
    const timer = setInterval(() => {
      if (visible()) setRound((n) => n + 1)
    }, pollMs)
    const onShow = () => {
      if (visible()) setRound((n) => n + 1)
    }
    document.addEventListener('visibilitychange', onShow)
    return () => {
      clearInterval(timer)
      document.removeEventListener('visibilitychange', onShow)
    }
  }, [pollMs])

  return { overview, missing, error, fetchedAt, updated, retry }
}
