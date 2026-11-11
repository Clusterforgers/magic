/**
 * The viewer's own order of the hand dock, shared by the dock and the table menu: the auto sort is one of the account's
 * preferences (`play/flowPrefs`: on the server for an account, in `localStorage` for a guest), the same in every game; a
 * dragged custom order lives in `sessionStorage` per game, so it survives a reload of the tab. Every access to the
 * session storage can fail (blocked storage, a private window, a full quota) and the order then lives for the page.
 */
import { useCallback, useMemo, useSyncExternalStore } from 'react'
import { flowPrefs, saveHandSort, useFlowPrefs } from '@/play/flowPrefs'
import { manaValue } from '@/features/table/prompt/payment'
import type { HandGroup, HandSortBy, ObjectId, ObjectView } from '@/protocol/generated'
import { mergeCustom, moveCard, parseCustom, sortHand } from './handOrder'
import type { HandFacts, HandOrderMode } from './handOrder'

/** The `sessionStorage` key of a game's custom order. */
export function handOrderKey(game: string): string {
  return `magic.handOrder.${game}`
}

function read(key: string): string | null {
  try {
    return window.sessionStorage.getItem(key)
  } catch {
    return null
  }
}

function write(key: string, value: string | null): void {
  try {
    if (value === null) window.sessionStorage.removeItem(key)
    else window.sessionStorage.setItem(key, value)
  } catch {
    // Kept for this page only.
  }
}

const listeners = new Set<() => void>()
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

/** The custom order of `game`, or null while an auto sort holds. */
export function customOrder(game: string): ObjectId[] | null {
  if (!customs.has(game)) customs.set(game, parseCustom(read(handOrderKey(game))))
  return customs.get(game) ?? null
}

/** Drops the custom order of `game`, so the auto sort holds again. */
function dropCustom(game: string): void {
  customs.set(game, null)
  write(handOrderKey(game), null)
  changed()
}

/** Picks an auto sort for every game and drops the custom order of `game`. */
export function chooseHandSort(game: string, by: HandSortBy): void {
  saveHandSort({ ...flowPrefs().handSort, by })
  dropCustom(game)
}

/** Sets the order of the groups of "By type" for every game, which makes it the sort, and drops the custom order of `game`. */
export function orderHandGroups(game: string, groups: HandGroup[]): void {
  saveHandSort({ by: 'Groups', groups })
  dropCustom(game)
}

/** Keeps `ids` as the custom order of `game`. */
export function setCustomOrder(game: string, ids: ObjectId[]): void {
  customs.set(game, ids)
  write(handOrderKey(game), JSON.stringify(ids))
  changed()
}

/** Forgets the cached custom orders, so the next read goes to storage again (tests). */
export function resetHandOrder(): void {
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

/** The order in force in `game`, the groups of "By type", and the ways to change them, redrawn when either changes. */
export function useHandMode(game: string): {
  mode: HandOrderMode
  groups: readonly HandGroup[]
  choose(by: HandSortBy): void
  order(groups: HandGroup[]): void
} {
  const custom = useSyncExternalStore(subscribe, () => customOrder(game))
  const { handSort } = useFlowPrefs()
  const choose = useCallback((by: HandSortBy) => chooseHandSort(game, by), [game])
  const order = useCallback((groups: HandGroup[]) => orderHandGroups(game, groups), [game])
  return { mode: custom === null ? handSort.by : 'custom', groups: handSort.groups, choose, order }
}

/** The order the dock draws, how it came about, and the ways to change it. */
export interface HandOrder {
  /** The hand's ids in drawing order. */
  ids: ObjectId[]
  mode: HandOrderMode
  /** Picks an auto sort, which drops the game's custom order. */
  choose(by: HandSortBy): void
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
  const { handSort } = useFlowPrefs()
  const ids = useMemo(
    () => (custom === null ? sortHand(cards, factsOf(objects), handSort) : mergeCustom(custom, cards)),
    [cards, objects, handSort, custom],
  )
  const choose = useCallback((by: HandSortBy) => chooseHandSort(game, by), [game])
  const move = useCallback(
    (from: number, to: number) => setCustomOrder(game, moveCard(ids, from, to)),
    [game, ids],
  )
  return { ids, mode: custom === null ? handSort.by : 'custom', choose, move }
}
