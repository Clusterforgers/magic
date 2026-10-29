import { useEffect, useRef, useState } from 'react'
import type { CardsApi } from '@/net/cards'
import type { FaceTexts } from './model'

/** Cards read at once; the card endpoints are not rationed, but a long list should not flood the server. */
const AT_ONCE = 4

type Reader = (oracleId: string) => Promise<readonly string[]>

/** A reader of `GET /api/cards/{oracle_id}` that asks for each card once, `AT_ONCE` at a time; a failure is asked again. */
function createReader(api: Pick<CardsApi, 'get'>): Reader {
  const known = new Map<string, Promise<readonly string[]>>()
  const waiting: Array<() => void> = []
  let running = 0

  const pump = (): void => {
    while (running < AT_ONCE) {
      const next = waiting.shift()
      if (next === undefined) return
      running += 1
      next()
    }
  }

  return (oracleId) => {
    const before = known.get(oracleId)
    if (before !== undefined) return before
    const read = new Promise<readonly string[]>((resolve, reject) => {
      waiting.push(() => {
        api
          .get(oracleId)
          .then(
            (detail) => {
              const faces = detail.faces.map((face) => face.oracle_text)
              resolve(faces.length > 0 ? faces : [detail.oracle_text])
            },
            (error: unknown) => {
              known.delete(oracleId)
              reject(error instanceof Error ? error : new Error('could not read the card'))
            },
          )
          .finally(() => {
            running -= 1
            pump()
          })
      })
      pump()
    })
    known.set(oracleId, read)
    return read
  }
}

// One reader a client, so a card read for the precon dialog is not read again for the list.
const readers = new WeakMap<object, Reader>()

function readerFor(api: Pick<CardsApi, 'get'>): Reader {
  const found = readers.get(api)
  if (found !== undefined) return found
  const made = createReader(api)
  readers.set(api, made)
  return made
}

/** What `useCardTexts` has read. */
export interface CardTexts {
  /** The oracle text of each card read, one string per face. A card that could not be read has none. */
  faces: FaceTexts
  /** Some card asked for is still being read. */
  pending: boolean
}

/**
 * Reads the oracle text of `ids` from the card endpoint, in the order given, to quote the clauses a deck check leaves
 * to the table (a span counts characters of a face's text). A card that cannot be read is left out, so its clauses
 * show without the quote.
 */
export function useCardTexts(api: Pick<CardsApi, 'get'>, ids: readonly string[]): CardTexts {
  const [read, setRead] = useState<ReadonlyMap<string, readonly string[] | null>>(new Map())
  const asked = useRef(new Set<string>())
  const key = ids.join('\n')

  useEffect(() => {
    const fresh = key === '' ? [] : key.split('\n').filter((id) => !asked.current.has(id))
    const reader = readerFor(api)
    for (const id of fresh) {
      asked.current.add(id)
      reader(id).then(
        (faces) => setRead((before) => new Map(before).set(id, faces)),
        () => setRead((before) => new Map(before).set(id, null)),
      )
    }
  }, [api, key])

  const faces = new Map<string, readonly string[]>()
  let pending = false
  for (const id of ids) {
    const found = read.get(id)
    if (found === undefined) pending = true
    else if (found !== null) faces.set(id, found)
  }
  return { faces, pending }
}
