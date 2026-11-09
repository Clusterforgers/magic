/**
 * The rules text of a card, read once per card from `GET /api/cards/{oracle_id}` and shared by every copy, so a prompt
 * can word an effect (a replacement effect to choose) the way the card does, and a search can match it.
 */
import { useMemo, useSyncExternalStore } from 'react'
import { cardsApi } from '@/net/cards'
import type { CardDetail, ObjectView } from '@/protocol/generated'

/** Reads one card's detail; the cards API by default. */
export type OracleFetcher = (oracleId: string) => Promise<CardDetail>

/** Cards read at once, so a search of a whole library does not ask for a hundred cards in one go. */
const AT_ONCE = 6

let fetcher: OracleFetcher = (id) => cardsApi.get(id)
const known = new Map<string, readonly string[] | 'loading' | 'unavailable'>()
const listeners = new Set<() => void>()
const waiting: string[] = []
let running = 0
let version = 0

/** Replaces the fetcher and forgets what was read (tests, the dev table). */
export function setOracleFetcher(next: OracleFetcher): void {
  fetcher = next
  known.clear()
  waiting.length = 0
  running = 0
  version += 1
}

function ask(oracleId: string): void {
  known.set(oracleId, 'loading')
  waiting.push(oracleId)
  pump()
}

function pump(): void {
  while (running < AT_ONCE) {
    const oracleId = waiting.shift()
    if (oracleId === undefined) return
    running += 1
    const read = fetcher
    const settle = (value: readonly string[] | 'unavailable'): void => {
      // The answer of a fetcher replaced since belongs to what was forgotten.
      if (read !== fetcher) return
      running -= 1
      known.set(oracleId, value)
      version += 1
      for (const listener of listeners) listener()
      pump()
    }
    read(oracleId).then(
      (detail) => settle(detail.faces.map((face) => face.oracle_text)),
      () => settle('unavailable'),
    )
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

const snapshot = (): number => version

/** The rules text of the face `object` shows from what is read so far, asking for it once; null while unknown. */
function textOf(object: ObjectView | undefined): string | null {
  const oracleId = object?.identity?.oracle_id
  if (object === undefined || oracleId === undefined) return null
  const entry = known.get(oracleId)
  if (entry === undefined) {
    ask(oracleId)
    return null
  }
  if (entry === 'loading' || entry === 'unavailable') return null
  const text = entry[object.face ?? 0] ?? entry[0] ?? ''
  return text.trim() === '' ? null : text
}

/**
 * The rules text of the face an object shows, or null while it loads, when the card could not be read and for an
 * object with no card (a token, an effect, a view without identity).
 */
export function useOracleText(object: ObjectView | undefined): string | null {
  useSyncExternalStore(subscribe, snapshot, snapshot)
  return textOf(object)
}

/**
 * The rules text of each of `objects` read so far (as `useOracleText`), the rest asked for a few at a time; the map
 * is new each time a text comes in.
 */
export function useOracleTexts(
  objects: readonly (ObjectView | undefined)[],
): ReadonlyMap<ObjectView, string> {
  const seen = useSyncExternalStore(subscribe, snapshot, snapshot)
  return useMemo(() => {
    // Each read moves `seen`, which is what makes the map new.
    void seen
    const texts = new Map<ObjectView, string>()
    for (const object of objects) {
      const text = textOf(object)
      if (object !== undefined && text !== null) texts.set(object, text)
    }
    return texts
  }, [objects, seen])
}
