/**
 * The printed faces of the cards on the battlefield, read once per card from `GET /api/cards/{oracle_id}` and shared
 * by every copy, so a picture can show where power, toughness or loyalty moved away from the printed value and name
 * the front of a card that shows its back.
 */
import { useSyncExternalStore } from 'react'
import { cardsApi } from '@/net/cards'
import type { CardDetail, ObjectView } from '@/protocol/generated'
import { showsBackFace } from './cardModel'
import type { Printed, PrintedStats } from './overlay'

/** Reads one card's detail; the cards API by default. */
export type DetailFetcher = (oracleId: string) => Promise<CardDetail>

/** One printed face: its stats and its name. */
type PrintedFace = PrintedStats & { name: string }

let fetcher: DetailFetcher = (id) => cardsApi.get(id)
const known = new Map<string, readonly PrintedFace[] | 'loading' | 'unavailable'>()
const listeners = new Set<() => void>()
let version = 0

/** Replaces the fetcher and forgets what was read (tests, the dev table). */
export function setDetailFetcher(next: DetailFetcher): void {
  fetcher = next
  known.clear()
}

function ask(oracleId: string): void {
  known.set(oracleId, 'loading')
  const settle = (value: readonly PrintedFace[] | 'unavailable'): void => {
    known.set(oracleId, value)
    version += 1
    for (const listener of listeners) listener()
  }
  fetcher(oracleId).then(
    (detail) =>
      settle(
        detail.faces.map(({ name, power, toughness, loyalty }) => ({ name, power, toughness, loyalty })),
      ),
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

/** The printed stats of the face a permanent shows; 'none' off the battlefield and for an object with no card. */
export function usePrinted(object: ObjectView): Printed {
  useSyncExternalStore(subscribe, snapshot, snapshot)
  const oracleId = object.identity?.oracle_id
  if (oracleId === undefined || object.zone !== 'Battlefield') return 'none'
  const entry = known.get(oracleId)
  if (entry === undefined) {
    ask(oracleId)
    return 'loading'
  }
  if (entry === 'loading' || entry === 'unavailable') return entry
  return entry[object.face ?? 0] ?? entry[0] ?? 'unavailable'
}

/** The name of the front face of a card a permanent shows as its back face; undefined for any other, and while unknown. */
export function useFrontFaceName(object: ObjectView | undefined): string | undefined {
  useSyncExternalStore(subscribe, snapshot, snapshot)
  const oracleId = object?.identity?.oracle_id
  if (object === undefined || oracleId === undefined || !showsBackFace(object)) return undefined
  const entry = known.get(oracleId)
  if (entry === undefined) {
    ask(oracleId)
    return undefined
  }
  return entry === 'loading' || entry === 'unavailable' ? undefined : entry[0]?.name
}
