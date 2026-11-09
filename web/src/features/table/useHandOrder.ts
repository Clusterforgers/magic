/**
 * The viewer's own order of the hand dock, shared by the dock and the table menu: the auto sort is remembered in
 * `localStorage` for every game, a dragged custom order in `sessionStorage` per game, so it survives a reload of the
 * tab. Every access can fail (blocked storage, a private window, a full quota) and the order then lives for the page.
 */
import { useCallback, useMemo, useSyncExternalStore } from 'react'
import { manaValue } from '@/features/table/prompt/payment'
import type { ObjectId, ObjectView } from '@/protocol/generated'
import { isHandSort, mergeCustom, moveCard, parseCustom, sortHand } from './handOrder'
import type { HandFacts, HandOrderMode, HandSort } from './handOrder'

/** The `localStorage` key of the auto sort. */
export const HAND_SORT_KEY = 'magic.handSort'

/** The `sessionStorage` key of a game's custom order. */
export function handOrderKey(game: string): string {
  return `magic.handOrder.${game}`
}

function read(kind: 'localStorage' | 'sessionStorage', key: string): string | null {
  try {
    return window[kind].getItem(key)
  } catch {
    return null
  }
}

function write(kind: 'localStorage' | 'sessionStorage', key: string, value: string | null): void {
  try {
    if (value === null) window[kind].removeItem(key)
    else window[kind].setItem(key, value)
  } catch {
    // Kept for this page only.
  }
}

const listeners = new Set<() => void>()
let sort: HandSort | null = null
const customs = new Map<string, ObjectId[] | null>()

function changed(): void {
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** The auto sort now: the stored one, "As drawn" by default. */
export function handSort(): HandSort {
  if (sort === null) {
    const kept = read('localStorage', HAND_SORT_KEY)
    sort = isHandSort(kept) ? kept : 'drawn'
  }
  return sort
}

/** The custom order of `game`, or null while an auto sort holds. */
export function customOrder(game: string): ObjectId[] | null {
  if (!customs.has(game)) customs.set(game, parseCustom(read('sessionStorage', handOrderKey(game))))
  return customs.get(game) ?? null
}

/** Picks an auto sort for every game and drops the custom order of `game`. */
export function chooseHandSort(game: string, next: HandSort): void {
  sort = next
  customs.set(game, null)
  write('localStorage', HAND_SORT_KEY, next)
  write('sessionStorage', handOrderKey(game), null)
  changed()
}

/** Keeps `ids` as the custom order of `game`. */
export function setCustomOrder(game: string, ids: ObjectId[]): void {
  customs.set(game, ids)
  write('sessionStorage', handOrderKey(game), JSON.stringify(ids))
  changed()
}

/** Forgets the cached choices, so the next read goes to storage again (tests). */
export function resetHandOrder(): void {
  sort = null
  customs.clear()
}

/** The facts a sort reads, from the objects of the view. */
export function factsOf(objects: Readonly<Record<ObjectId, ObjectView>>) {
  return (id: ObjectId): HandFacts | undefined => {
    const object = objects[id]
    if (object === undefined) return undefined
    return { types: object.types, manaValue: manaValue(object.mana_cost ?? []), name: object.name }
  }
}

/** The order in force in `game` and the way to pick an auto sort, redrawn when either changes. */
export function useHandMode(game: string): { mode: HandOrderMode; choose(sort: HandSort): void } {
  const custom = useSyncExternalStore(subscribe, () => customOrder(game))
  const auto = useSyncExternalStore(subscribe, handSort)
  const choose = useCallback((next: HandSort) => chooseHandSort(game, next), [game])
  return { mode: custom === null ? auto : 'custom', choose }
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

/** The viewer's order of `cards` (the server's hand) in `game`. */
export function useHandOrder(
  game: string,
  cards: readonly ObjectId[],
  objects: Readonly<Record<ObjectId, ObjectView>>,
): HandOrder {
  const custom = useSyncExternalStore(subscribe, () => customOrder(game))
  const auto = useSyncExternalStore(subscribe, handSort)
  const ids = useMemo(
    () => (custom === null ? sortHand(cards, factsOf(objects), auto) : mergeCustom(custom, cards)),
    [cards, objects, auto, custom],
  )
  const choose = useCallback((next: HandSort) => chooseHandSort(game, next), [game])
  const move = useCallback(
    (from: number, to: number) => setCustomOrder(game, moveCard(ids, from, to)),
    [game, ids],
  )
  return { ids, mode: custom === null ? auto : 'custom', choose, move }
}
