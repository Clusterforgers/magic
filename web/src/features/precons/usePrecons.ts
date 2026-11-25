import { useCallback, useEffect, useState } from 'react'
import type { PreconsApi } from '@/net/precons'
import type { PreconListItem } from '@/protocol/generated'

/** How long after a list the server is still judging (`PreconList.judging`) the list is read again. */
export const JUDGING_RETRY_MS = 5_000

/** The list of precons as the page sees it. */
export type PreconsState =
  | { status: 'loading' }
  | { status: 'ready'; precons: PreconListItem[] }
  | { status: 'failed'; error: unknown }

type Settled = Exclude<PreconsState, { status: 'loading' }>

/**
 * Loads `GET /api/precons` once, and again when `reload` is called. A list the server is still judging is shown and
 * read again every `JUDGING_RETRY_MS` until it is current; a failed re-read keeps the list shown.
 */
export function usePrecons(api: Pick<PreconsApi, 'list'>): { state: PreconsState; reload: () => void } {
  const [round, setRound] = useState(0)
  const [pass, setPass] = useState(0)
  // Tagged with the round it answers, so a new round reads as loading without resetting state inside the effect.
  const [settled, setSettled] = useState<{ round: number; state: Settled } | null>(null)
  const reload = useCallback(() => setRound((n) => n + 1), [])

  useEffect(() => {
    const controller = new AbortController()
    let retry: ReturnType<typeof setTimeout> | undefined
    api.list(controller.signal).then(
      (list) => {
        if (controller.signal.aborted) return
        setSettled({ round, state: { status: 'ready', precons: list.precons } })
        if (list.judging) retry = setTimeout(() => setPass((n) => n + 1), JUDGING_RETRY_MS)
      },
      (error: unknown) => {
        if (controller.signal.aborted) return
        setSettled((before) =>
          before !== null && before.round === round && before.state.status === 'ready'
            ? before
            : { round, state: { status: 'failed', error } },
        )
      },
    )
    return () => {
      controller.abort()
      clearTimeout(retry)
    }
  }, [api, round, pass])

  return {
    state: settled !== null && settled.round === round ? settled.state : { status: 'loading' },
    reload,
  }
}
