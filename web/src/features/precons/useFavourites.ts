import { useCallback, useEffect, useMemo, useState } from 'react'
import { describeDeckError } from '@/features/decks/errors'
import { DeckError } from '@/net/decks'
import type { FavouritesApi } from '@/net/favourites'

/** The account's favourite precons and the press that changes them. */
export interface Favourites {
  /** The codes of the starred precons; empty until the list has loaded and for a guest. */
  codes: ReadonlySet<string>
  /** Whether the precon is starred. */
  has(code: string): boolean
  /** The change of this precon is on its way to the server. */
  isPending(code: string): boolean
  /** Stars the precon or takes the star back: the star changes at once and goes back if the server refuses. */
  toggle(code: string): void
  /** Why the last change failed; null after a change that worked and before any failure. */
  problem: string | null
  /** Forgets the problem. */
  clearProblem(): void
}

/** What to tell a player about a star that could not be set or taken back. */
export function favouriteWords(error: unknown, starring: boolean): string {
  if (error instanceof DeckError) {
    if (error.code === 'not_found')
      return 'That precon is no longer on the server, so it cannot be a favourite.'
    if (error.code === 'rate_limited')
      return 'You changed favourites too often. Wait a moment, then try again.'
    if (error.code === 'unauthenticated') return 'Your session has ended. Sign in again to keep favourites.'
  }
  return `${starring ? 'The star was not saved' : 'The star was not removed'}. ${describeDeckError(error)}`
}

const without = (set: ReadonlySet<string>, code: string): Set<string> => {
  const next = new Set(set)
  next.delete(code)
  return next
}

/**
 * The favourite precons of the signed-in account (`GET /api/favourites/precons`), loaded once when `enabled` and kept
 * for the life of the page (a list that cannot be had leaves no stars and no message). A guest (`enabled` false) has none and asks for nothing. A press is optimistic: the star
 * shows at once and goes back if the server refuses, with the reason in `problem`.
 */
export function useFavourites(api: FavouritesApi, enabled: boolean): Favourites {
  const [codes, setCodes] = useState<ReadonlySet<string>>(new Set())
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set())
  const [problem, setProblem] = useState<string | null>(null)

  useEffect(() => {
    if (!enabled) return
    let stale = false
    api.list().then(
      (list) => {
        if (!stale) setCodes(new Set(list))
      },
      () => {
        // The stars are a convenience: without them the lists show no star and nothing else is lost.
      },
    )
    return () => {
      stale = true
    }
  }, [api, enabled])

  const toggle = useCallback(
    (code: string): void => {
      if (!enabled || pending.has(code)) return
      const starred = codes.has(code)
      setCodes((was) => (starred ? without(was, code) : new Set(was).add(code)))
      setPending((was) => new Set(was).add(code))
      const done = starred ? api.unstar(code) : api.star(code)
      done
        .then(
          () => setProblem(null),
          (error: unknown) => {
            setCodes((was) => (starred ? new Set(was).add(code) : without(was, code)))
            setProblem(favouriteWords(error, !starred))
          },
        )
        .finally(() => setPending((was) => without(was, code)))
    },
    [api, codes, enabled, pending],
  )

  const shown = enabled ? codes : EMPTY
  return useMemo(
    () => ({
      codes: shown,
      has: (code) => shown.has(code),
      isPending: (code) => pending.has(code),
      toggle,
      problem,
      clearProblem: () => setProblem(null),
    }),
    [shown, pending, toggle, problem],
  )
}

const EMPTY: ReadonlySet<string> = new Set()
