import { useCallback, useEffect, useMemo, useState } from 'react'
import { describeDeckError } from '@/features/decks/errors'
import type { DecksApi } from '@/net/decks'
import type { CheckRequest, DeckReport } from '@/protocol/generated'
import { checkKey, checkRequestOf, totalOf } from './deckEdit'
import type { Draft } from './deckEdit'

/** How long the draft must rest before it is checked, in milliseconds. */
export const CHECK_PAUSE_MS = 600

/**
 * How the report relates to the draft on screen: `current` is the report of this very list, `checking` says a check of
 * it is on its way (after the pause), and `failed` that the check could not be made.
 */
export type CheckStatus = 'current' | 'checking' | 'failed'

/** What `useLiveCheck` gives the deck pane. */
export interface LiveCheck {
  /** The report to show: the saved deck's, or the newest check of a draft. */
  report: DeckReport
  /** Where `report` comes from: the saved deck, or a check of a draft (that of an earlier list while `status` is not `current`). */
  source: 'saved' | 'draft'
  status: CheckStatus
  /** What went wrong, in words, while `status` is `failed`. */
  error: string | null
  /** Checks the draft again after a failure, without waiting out the pause. */
  retry: () => void
}

interface Rested {
  key: string
  request: CheckRequest
}

/**
 * The deck check that follows the draft. While the draft's cards or format differ from the saved deck's, the draft is
 * posted to `POST /api/decks/check` once it has rested for `pauseMs`; a newer edit cancels the request before it. The
 * report of the saved deck shows until a check of the draft comes back, the last good report stays through a failure,
 * and the saved report returns the moment the draft is the saved list again. A rename or new notes change nothing,
 * and a deck with no cards is not sent.
 */
export function useLiveCheck(
  api: DecksApi,
  saved: { draft: Draft; report: DeckReport },
  draft: Draft,
  pauseMs: number = CHECK_PAUSE_MS,
): LiveCheck {
  const key = checkKey(draft)
  const wanted = key !== checkKey(saved.draft) && totalOf(draft) > 0
  const { format, lines } = draft
  const request = useMemo(() => checkRequestOf({ format, lines }), [format, lines])
  const [rested, setRested] = useState<Rested | null>(null)
  const [live, setLive] = useState<{ key: string; report: DeckReport } | null>(null)
  const [failure, setFailure] = useState<{ key: string; round: number; message: string } | null>(null)
  const [round, setRound] = useState(0)

  // Back at the saved list, whatever was learned of the drafts is behind it.
  if (!wanted && (rested !== null || live !== null || failure !== null)) {
    setRested(null)
    setLive(null)
    setFailure(null)
  }

  useEffect(() => {
    if (!wanted || rested?.key === key) return
    const timer = window.setTimeout(() => setRested({ key, request }), pauseMs)
    return () => window.clearTimeout(timer)
  }, [wanted, rested, key, request, pauseMs])

  useEffect(() => {
    if (!wanted || rested === null || rested.key !== key) return
    const controller = new AbortController()
    let stale = false
    api.check(rested.request, controller.signal).then(
      (report) => {
        if (!stale) setLive({ key, report })
      },
      (error: unknown) => {
        if (!stale) setFailure({ key, round, message: describeDeckError(error) })
      },
    )
    return () => {
      stale = true
      controller.abort()
    }
  }, [api, wanted, rested, key, round])

  const retry = useCallback(() => setRound((n) => n + 1), [])
  const report = wanted && live !== null ? live.report : saved.report
  const source = wanted && live !== null ? 'draft' : 'saved'
  if (!wanted || live?.key === key) return { report, source, status: 'current', error: null, retry }
  if (failure?.key === key && failure.round === round) {
    return { report, source, status: 'failed', error: failure.message, retry }
  }
  return { report, source, status: 'checking', error: null, retry }
}
