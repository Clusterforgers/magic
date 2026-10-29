import { useCallback, useEffect, useState } from 'react'
import type { PreconsApi } from '@/net/precons'
import type { PreconListItem } from '@/protocol/generated'

/** The list of precons as the page sees it. */
export type PreconsState =
  | { status: 'loading' }
  | { status: 'ready'; precons: PreconListItem[] }
  | { status: 'failed'; error: unknown }

type Settled = Exclude<PreconsState, { status: 'loading' }>

/** Loads `GET /api/precons` once, and again when `reload` is called. */
export function usePrecons(api: Pick<PreconsApi, 'list'>): { state: PreconsState; reload: () => void } {
  const [round, setRound] = useState(0)
  // Tagged with the round it answers, so a new round reads as loading without resetting state inside the effect.
  const [settled, setSettled] = useState<{ round: number; state: Settled } | null>(null)
  const reload = useCallback(() => setRound((n) => n + 1), [])

  useEffect(() => {
    const controller = new AbortController()
    api.list(controller.signal).then(
      (list) => {
        if (!controller.signal.aborted)
          setSettled({ round, state: { status: 'ready', precons: list.precons } })
      },
      (error: unknown) => {
        if (!controller.signal.aborted) setSettled({ round, state: { status: 'failed', error } })
      },
    )
    return () => controller.abort()
  }, [api, round])

  return {
    state: settled !== null && settled.round === round ? settled.state : { status: 'loading' },
    reload,
  }
}
