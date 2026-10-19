import { useCallback, useEffect, useRef, useState } from 'react'
import { useTabHidden } from '@/hooks/useTabHidden'
import type { Api, LobbyListing } from '@/net/api'
import { LobbyError } from '@/net/lobbyErrors'

/** How long after an answer the list is asked for again while the tab is visible, in milliseconds. */
export const POLL_MS = 5_000

/** What the page knows about the open games. */
export interface OpenGames {
  /** The last listing that arrived; kept through failures, so the rows can stay up (dimmed) while an error shows. */
  listing: LobbyListing | null
  /** The failure of the latest request, until a later one succeeds. */
  error: Error | null
  /** True while a request asked for with the Refresh or Retry button is on its way. */
  refreshing: boolean
  /** Asks for the list now, and then goes on polling from there. */
  refresh(): void
}

interface Held {
  listing: LobbyListing | null
  error: Error | null
  refreshing: boolean
}

const asError = (error: unknown): Error => (error instanceof Error ? error : new Error(String(error)))

/**
 * Keeps the open games current: asks at once, then 5 s after each answer, with the `ETag` of the listing it holds, so
 * a 304 hands back the same object and changes nothing. It pauses while the tab is hidden and asks again the moment
 * the tab is visible. A failure keeps the listing; a 429 waits as long as the server says. Only one request counts at
 * a time: an answer that a newer request or the end of the page has overtaken is dropped.
 */
export function useOpenGames(api: Pick<Api, 'listLobbies'>): OpenGames {
  const hidden = useTabHidden()
  const [held, setHeld] = useState<Held>({ listing: null, error: null, refreshing: false })
  const known = useRef<LobbyListing | undefined>(undefined)
  const round = useRef(0)
  const timer = useRef<number | undefined>(undefined)

  const load = useCallback(
    async function run(): Promise<void> {
      window.clearTimeout(timer.current)
      round.current += 1
      const mine = round.current
      let wait = POLL_MS
      try {
        const next = await api.listLobbies(known.current)
        if (mine !== round.current) return
        known.current = next
        // A 304 returns the listing it was given: the same object keeps the same state, so nothing renders again.
        setHeld((was) =>
          was.listing === next && was.error === null && !was.refreshing
            ? was
            : { listing: next, error: null, refreshing: false },
        )
      } catch (failure) {
        if (mine !== round.current) return
        const error = asError(failure)
        if (error instanceof LobbyError && error.retryAfter !== null) {
          wait = Math.max(wait, error.retryAfter * 1000)
        }
        setHeld((was) => ({ listing: was.listing, error, refreshing: false }))
      }
      timer.current = window.setTimeout(() => void run(), wait)
    },
    [api],
  )

  useEffect(() => {
    if (hidden) return
    void load()
    return () => {
      round.current += 1
      window.clearTimeout(timer.current)
    }
  }, [hidden, load])

  const refresh = useCallback(() => {
    setHeld((was) => (was.refreshing ? was : { ...was, refreshing: true }))
    void load()
  }, [load])
  return { listing: held.listing, error: held.error, refreshing: held.refreshing, refresh }
}
