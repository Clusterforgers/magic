import { useCallback, useEffect, useRef, useState } from 'react'
import type { DecksApi } from '@/net/decks'
import { verdictOf } from './verdict'
import type { Verdict } from './verdict'

/** Decks checked at once; the server counts every check against the address. */
const AT_ONCE = 3

/** One deck's check at this table. */
export type VerdictState =
  { status: 'checking' } | { status: 'ready'; verdict: Verdict } | { status: 'failed' }

/** The checks the chooser holds, by deck id, and the way to ask again for one that failed. */
export interface Verdicts {
  of: ReadonlyMap<string, VerdictState>
  retry(id: string): void
}

/**
 * Checks each wanted deck at this game (`POST /api/decks/check` with `game`), three at a time, once per deck: a deck
 * that was asked about keeps its answer while the player moves between lists, and a failed one is asked again only
 * on `retry`.
 */
export function useVerdicts(api: Pick<DecksApi, 'check'>, game: string, wanted: readonly string[]): Verdicts {
  const [of, setOf] = useState<ReadonlyMap<string, VerdictState>>(new Map())
  const asked = useRef(new Set<string>())
  const queue = useRef<string[]>([])
  const running = useRef(0)

  const set = useCallback((id: string, state: VerdictState): void => {
    setOf((before) => new Map(before).set(id, state))
  }, [])

  const pump = useCallback(
    function run(): void {
      while (running.current < AT_ONCE) {
        const id = queue.current.shift()
        if (id === undefined) return
        running.current += 1
        void api
          .check({ deck: id, format: null, cards: null, game })
          .then(
            (report) => set(id, { status: 'ready', verdict: verdictOf(report) }),
            () => set(id, { status: 'failed' }),
          )
          .finally(() => {
            running.current -= 1
            run()
          })
      }
    },
    [api, game, set],
  )

  const key = wanted.join('\n')
  useEffect(() => {
    const fresh = key === '' ? [] : key.split('\n').filter((id) => !asked.current.has(id))
    if (fresh.length === 0) return
    for (const id of fresh) asked.current.add(id)
    setOf((before) => {
      const next = new Map(before)
      for (const id of fresh) next.set(id, { status: 'checking' })
      return next
    })
    queue.current.push(...fresh)
    pump()
  }, [key, pump])

  const retry = useCallback(
    (id: string): void => {
      set(id, { status: 'checking' })
      queue.current.push(id)
      pump()
    },
    [pump, set],
  )

  return { of, retry }
}
