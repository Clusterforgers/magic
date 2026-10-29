import { useCallback, useEffect, useState } from 'react'
import { describeDeckError } from '@/features/decks/errors'
import { previewFromDeck, previewFromPrecon } from '@/features/decks/preview/model'
import type { DeckPreviewData } from '@/features/decks/preview/model'
import { describePreconError } from '@/features/precons/errors'
import type { DecksApi } from '@/net/decks'
import type { PreconsApi } from '@/net/precons'
import type { DeckEntry } from './deckEntries'

/** The cards of the deck being previewed. */
export type DeckDetail =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'ready'; data: DeckPreviewData }
  | { status: 'failed'; message: string }

/** The preview's data and the way to ask again after a failure. */
export interface DeckDetails {
  /** What is known of a deck: its view once read, loading while it is asked for, idle for a built-in deck. */
  of(entry: DeckEntry | null): DeckDetail
  /** The commanders and cards of a deck whose view was read, for rows that have no picture of their own. */
  known(id: string): DeckPreviewData | undefined
  retry(): void
}

/**
 * The cards of the selected deck: `GET /api/decks/{id}` for a stored deck and `GET /api/precons/{code}` for a precon,
 * asked for when `asked` is the deck (the chooser passes the selection once it has rested) and kept for the life of
 * the page (the server counts every request against its budget). A built-in deck has no card data and stays idle.
 */
export function useDeckDetails(
  decks: Pick<DecksApi, 'get'>,
  precons: Pick<PreconsApi, 'get'>,
  asked: DeckEntry | null,
): DeckDetails {
  const [views, setViews] = useState<ReadonlyMap<string, DeckPreviewData>>(new Map())
  const [failed, setFailed] = useState<{ id: string; round: number; message: string } | null>(null)
  const [round, setRound] = useState(0)
  const retry = useCallback(() => setRound((n) => n + 1), [])
  const ask = asked !== null && asked.kind !== 'builtin' ? asked : null
  const cached = ask === null ? undefined : views.get(ask.id)

  useEffect(() => {
    if (ask === null || cached !== undefined) return
    const controller = new AbortController()
    const finish = (data: DeckPreviewData): void => {
      if (!controller.signal.aborted) setViews((was) => new Map(was).set(ask.id, data))
    }
    const fail = (message: string): void => {
      if (!controller.signal.aborted) setFailed({ id: ask.id, round, message })
    }
    if (ask.kind === 'precon') {
      precons.get(ask.id.slice('precon:'.length), controller.signal).then(
        (view) => finish(previewFromPrecon(view)),
        (error: unknown) => fail(describePreconError(error)),
      )
    } else {
      decks.get(ask.id).then(
        (view) => finish(previewFromDeck(view)),
        (error: unknown) => fail(describeDeckError(error)),
      )
    }
    return () => controller.abort()
  }, [ask, cached, decks, precons, round])

  return {
    of: (entry) => {
      if (entry === null || entry.kind === 'builtin') return { status: 'idle' }
      const read = views.get(entry.id)
      if (read !== undefined) return { status: 'ready', data: read }
      if (failed !== null && failed.id === entry.id && failed.round === round) {
        return { status: 'failed', message: failed.message }
      }
      return { status: 'loading' }
    },
    known: (id) => views.get(id),
    retry,
  }
}
