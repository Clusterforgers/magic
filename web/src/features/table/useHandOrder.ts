/**
 * The viewer's own order of the hand dock: the auto sort is remembered in `localStorage` for every game, a dragged
 * custom order in `sessionStorage` per game, so it survives a reload of the tab. Every access can fail (blocked
 * storage, a private window, a full quota) and the dock works without it.
 */
import { useCallback, useMemo, useState } from 'react'
import { manaValue } from '@/features/table/prompt/payment'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { HAND_SORTS, isHandSort, mergeCustom, moveCard, parseCustom, sortHand } from './handOrder'
import type { HandFacts, HandOrderMode, HandSort } from './handOrder'

/** The `localStorage` key of the auto sort. */
export const HAND_SORT_KEY = 'magic.handSort'

/** The `sessionStorage` key of a game's custom order. */
export function handOrderKey(game: string): string {
  return `magic.handOrder.${game}`
}

/** The few calls of a `Storage` the hand order uses. */
export interface HandStore {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
}

function storage(kind: 'localStorage' | 'sessionStorage'): HandStore | null {
  try {
    return window[kind]
  } catch {
    return null
  }
}

function read(store: HandStore | null, key: string): string | null {
  try {
    return store?.getItem(key) ?? null
  } catch {
    return null
  }
}

function write(store: HandStore | null, key: string, value: string | null): void {
  try {
    if (value === null) store?.removeItem(key)
    else store?.setItem(key, value)
  } catch {
    // The order still holds for this page; it is only not kept.
  }
}

/** The order the dock draws, how it came about, and the ways to change it. */
export interface HandOrder {
  /** The hand's ids in drawing order. */
  ids: ObjectId[]
  mode: HandOrderMode
  /** Picks an auto sort, which drops the game's custom order. */
  choose(sort: HandSort): void
  /** Moves the card at `from` to `to` of `ids`, which makes the order custom. */
  move(from: number, to: number): void
}

/** The menu label of a mode. */
export function modeLabel(mode: HandOrderMode): string {
  return HAND_SORTS.find((entry) => entry.value === mode)?.label ?? 'Custom'
}

/** The facts a sort reads, from the objects of the view. */
export function factsOf(objects: Readonly<Record<ObjectId, ObjectView>>) {
  return (id: ObjectId): HandFacts | undefined => {
    const object = objects[id]
    if (object === undefined) return undefined
    return { types: object.types, manaValue: manaValue(object.mana_cost ?? []), name: object.name }
  }
}

/** The viewer's order of `cards` (the server's hand) in `game`. */
export function useHandOrder(
  game: string,
  cards: readonly ObjectId[],
  objects: Readonly<Record<ObjectId, ObjectView>>,
  stores: { local: HandStore | null; session: HandStore | null } = {
    local: storage('localStorage'),
    session: storage('sessionStorage'),
  },
): HandOrder {
  const { local, session } = stores
  const [sort, setSort] = useState<HandSort>(() => {
    const kept = read(local, HAND_SORT_KEY)
    return isHandSort(kept) ? kept : 'drawn'
  })
  const [custom, setCustom] = useState<ObjectId[] | null>(() => parseCustom(read(session, handOrderKey(game))))
  const ids = useMemo(
    () => (custom === null ? sortHand(cards, factsOf(objects), sort) : mergeCustom(custom, cards)),
    [cards, objects, sort, custom],
  )
  const choose = useCallback(
    (next: HandSort) => {
      setSort(next)
      setCustom(null)
      write(local, HAND_SORT_KEY, next)
      write(session, handOrderKey(game), null)
    },
    [game, local, session],
  )
  const move = useCallback(
    (from: number, to: number) => {
      const next = moveCard(ids, from, to)
      setCustom(next)
      write(session, handOrderKey(game), JSON.stringify(next))
    },
    [game, ids, session],
  )
  return { ids, mode: custom === null ? sort : 'custom', choose, move }
}
