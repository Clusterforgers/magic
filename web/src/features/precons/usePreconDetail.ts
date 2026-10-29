import { useCallback, useEffect, useState } from 'react'
import type { PreconsApi } from '@/net/precons'
import type { PreconView } from '@/protocol/generated'
import { describePreconError } from './errors'

/** One precon's details as the dialog sees them. */
export type DetailState =
  { status: 'loading' } | { status: 'ready'; view: PreconView } | { status: 'failed'; message: string }

/**
 * Loads `GET /api/precons/{code}` for the precon the dialog shows (`null` for none). A view that was read stays for
 * the life of the page, so opening a precon again asks nothing: the server counts every request against its budget.
 */
export function usePreconDetail(
  api: Pick<PreconsApi, 'get'>,
  code: string | null,
): { state: DetailState; reload: () => void } {
  const [views, setViews] = useState<ReadonlyMap<string, PreconView>>(new Map())
  const [failed, setFailed] = useState<{ code: string; round: number; message: string } | null>(null)
  const [round, setRound] = useState(0)
  const reload = useCallback(() => setRound((n) => n + 1), [])
  const cached = code === null ? undefined : views.get(code)

  useEffect(() => {
    if (code === null || cached !== undefined) return
    const controller = new AbortController()
    api.get(code, controller.signal).then(
      (view) => {
        if (!controller.signal.aborted) setViews((was) => new Map(was).set(code, view))
      },
      (error: unknown) => {
        if (!controller.signal.aborted) setFailed({ code, round, message: describePreconError(error) })
      },
    )
    return () => controller.abort()
  }, [api, code, cached, round])

  if (cached !== undefined) return { state: { status: 'ready', view: cached }, reload }
  if (failed !== null && failed.code === code && failed.round === round) {
    return { state: { status: 'failed', message: failed.message }, reload }
  }
  return { state: { status: 'loading' }, reload }
}
