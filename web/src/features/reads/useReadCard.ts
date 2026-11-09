/** Loads one card's draft for reading and sends the verdict on it (docs/web.md, Reading cards). */
import { useCallback, useEffect, useRef, useState } from 'react'
import { DeckError } from '@/net/decks'
import type { ReadsApi } from '@/net/reads'
import type { ReadCard, ReadVerdict } from '@/protocol/generated'
import { forgetToReadCount } from './useToReadCount'

/** Where the card stands: loading, shown, without a draft to read (404), or failed. */
export type CardLoad =
  | { status: 'loading' }
  | { status: 'ready'; card: ReadCard }
  | { status: 'missing' }
  | { status: 'error'; error: unknown }

/** Where the verdict stands: none sent, on its way, refused (a `stale_read` among them), or taken. */
export type Sending =
  | { status: 'idle' }
  | { status: 'sending'; verdict: ReadVerdict }
  | { status: 'failed'; error: unknown }
  | { status: 'sent'; verdict: ReadVerdict }

/** True for the 409 of a draft that changed since the page loaded it. */
export const isStaleRead = (error: unknown): boolean =>
  error instanceof DeckError && error.code === 'stale_read'

/** The card, a way to load it again, and the verdict sender; `onJudged` hears of every verdict the server took. */
export function useReadCard(
  api: Pick<ReadsApi, 'get' | 'judge'>,
  oracleId: string,
  onJudged?: (card: ReadCard, verdict: ReadVerdict) => void,
) {
  const [load, setLoad] = useState<CardLoad>({ status: 'loading' })
  const [sending, setSending] = useState<Sending>({ status: 'idle' })
  const round = useRef(0)
  const judged = useRef(onJudged)
  useEffect(() => {
    judged.current = onJudged
  })

  const fetchCard = useCallback(() => {
    round.current += 1
    const mine = round.current
    api.get(oracleId).then(
      (card) => {
        if (round.current === mine) setLoad({ status: 'ready', card })
      },
      (error: unknown) => {
        if (round.current !== mine) return
        setLoad(
          error instanceof DeckError && error.status === 404
            ? { status: 'missing' }
            : { status: 'error', error },
        )
      },
    )
  }, [api, oracleId])

  useEffect(() => {
    fetchCard()
    return () => {
      round.current += 1
    }
  }, [fetchCard])

  const reload = useCallback(() => {
    setLoad({ status: 'loading' })
    setSending({ status: 'idle' })
    fetchCard()
  }, [fetchCard])

  const send = useCallback(
    (verdict: ReadVerdict, note: string | null) => {
      if (load.status !== 'ready' || sending.status === 'sending') return
      const mine = round.current
      setSending({ status: 'sending', verdict })
      api.judge(oracleId, { read_hash: load.card.read_hash, verdict, note }).then(
        (card) => {
          if (round.current !== mine) return
          setLoad({ status: 'ready', card })
          setSending({ status: 'sent', verdict })
          forgetToReadCount()
          judged.current?.(card, verdict)
        },
        (error: unknown) => {
          if (round.current === mine) setSending({ status: 'failed', error })
        },
      )
    },
    [api, oracleId, load, sending.status],
  )

  return { load, sending, send, reload }
}
