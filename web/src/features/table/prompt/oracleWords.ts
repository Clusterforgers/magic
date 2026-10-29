/**
 * The rules text of a card, read once per card from `GET /api/cards/{oracle_id}` and shared by every copy, so a prompt
 * can word an effect (a replacement effect to choose) the way the card does.
 */
import { useSyncExternalStore } from 'react'
import { cardsApi } from '@/net/cards'
import type { CardDetail, ObjectView } from '@/protocol/generated'

/** Reads one card's detail; the cards API by default. */
export type OracleFetcher = (oracleId: string) => Promise<CardDetail>

let fetcher: OracleFetcher = (id) => cardsApi.get(id)
const known = new Map<string, readonly string[] | 'loading' | 'unavailable'>()
const listeners = new Set<() => void>()
let version = 0

/** Replaces the fetcher and forgets what was read (tests, the dev table). */
export function setOracleFetcher(next: OracleFetcher): void {
  fetcher = next
  known.clear()
  version += 1
}

function ask(oracleId: string): void {
  known.set(oracleId, 'loading')
  const settle = (value: readonly string[] | 'unavailable'): void => {
    known.set(oracleId, value)
    version += 1
    for (const listener of listeners) listener()
  }
  fetcher(oracleId).then(
    (detail) => settle(detail.faces.map((face) => face.oracle_text)),
    () => settle('unavailable'),
  )
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

const snapshot = (): number => version

/**
 * The rules text of the face an object shows, or null while it loads, when the card could not be read and for an
 * object with no card (a token, an effect, a view without identity).
 */
export function useOracleText(object: ObjectView | undefined): string | null {
  useSyncExternalStore(subscribe, snapshot, snapshot)
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
