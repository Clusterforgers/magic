import { useCallback, useEffect, useState } from 'react'
import type { DecksApi } from '@/net/decks'
import { hasUnvalidated } from '../format'
import { describeDeckError, isDecksUnavailable, isSessionEnded } from '../errors'
import type { CheckedDeck, DeckRef } from './model'

/** Decks checked at once. */
const AT_ONCE = 4

/** What to list: the cards of every saved deck, or of one. */
export type Scope = { kind: 'all' } | { kind: 'deck'; id: string }

/** The checked decks as the page sees them. */
export type UnvalidatedState =
  | { status: 'loading'; done: number; total: number | null }
  | {
      status: 'ready'
      decks: CheckedDeck[]
      /** The decks whose check failed; the others are listed. */
      failed: DeckRef[]
      /** The decks the account holds. */
      saved: number
    }
  | { status: 'unavailable' }
  | { status: 'failed'; message: string; sessionEnded: boolean }

type Settled = Exclude<UnvalidatedState, { status: 'loading' }>

function failed(error: unknown): Settled {
  if (isDecksUnavailable(error)) return { status: 'unavailable' }
  return { status: 'failed', message: describeDeckError(error), sessionEnded: isSessionEnded(error) }
}

/** Runs `work` over `items`, `limit` at a time; each call settles itself, so one failure stops no other. */
async function pool<T>(items: readonly T[], limit: number, work: (item: T) => Promise<void>): Promise<void> {
  let next = 0
  const lane = async (): Promise<void> => {
    while (next < items.length) {
      const item = items[next]
      next += 1
      if (item !== undefined) await work(item)
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, lane))
}

/**
 * Loads the report of each deck in `scope`. One deck is a single `GET /api/decks/{id}`, which carries its name and the
 * report of the moment. All decks are `GET /api/decks` and then `POST /api/decks/check` for each deck the list
 * summarises with a card that is not Verified, four at a time; a deck that runs whole has nothing to list, so it is
 * not asked. A deck whose check fails is named in `failed` and the others still show. `reload` asks again.
 */
export function useUnvalidated(
  api: Pick<DecksApi, 'list' | 'get' | 'check'>,
  scope: Scope,
): { state: UnvalidatedState; reload: () => void } {
  const [round, setRound] = useState(0)
  const reload = useCallback(() => setRound((n) => n + 1), [])
  const scopeKey = scope.kind === 'all' ? '*' : scope.id
  const key = `${scopeKey}#${round}`
  const [progress, setProgress] = useState<{ key: string; done: number; total: number | null }>({
    key,
    done: 0,
    total: null,
  })
  const [settled, setSettled] = useState<{ key: string; state: Settled } | null>(null)

  useEffect(() => {
    const controller = new AbortController()
    const mine = (): boolean => !controller.signal.aborted
    const finish = (state: Settled): void => {
      if (mine()) setSettled({ key, state })
    }
    const load = async (): Promise<void> => {
      if (scopeKey !== '*') {
        const view = await api.get(scopeKey)
        finish({
          status: 'ready',
          decks: [{ id: view.id, name: view.name, cards: view.report.cards }],
          failed: [],
          saved: 1,
        })
        return
      }
      const list = await api.list()
      const wanted = list.filter((deck) => hasUnvalidated(deck.summary.counts))
      const checked = new Map<string, CheckedDeck>()
      const missed: DeckRef[] = []
      let done = 0
      if (mine()) setProgress({ key, done, total: wanted.length })
      await pool(wanted, AT_ONCE, async (deck) => {
        try {
          const report = await api.check({ deck: deck.id, format: null, cards: null }, controller.signal)
          checked.set(deck.id, { id: deck.id, name: deck.name, cards: report.cards })
        } catch (error) {
          if (!mine()) return
          // A session that ended or a server without decks fails every check alike; the first one says so.
          if (isSessionEnded(error) || isDecksUnavailable(error)) throw error
          missed.push({ id: deck.id, name: deck.name })
        }
        done += 1
        if (mine()) setProgress({ key, done, total: wanted.length })
      })
      finish({
        status: 'ready',
        decks: wanted.flatMap((deck) => checked.get(deck.id) ?? []),
        failed: missed,
        saved: list.length,
      })
    }
    load().catch((error: unknown) => finish(failed(error)))
    return () => controller.abort()
  }, [api, scopeKey, key])

  const state: UnvalidatedState =
    settled !== null && settled.key === key
      ? settled.state
      : {
          status: 'loading',
          done: progress.key === key ? progress.done : 0,
          total: progress.key === key ? progress.total : null,
        }
  return { state, reload }
}
