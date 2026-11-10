/**
 * The order of the precon list: newest first as the server sends it, by name, or the account's favorites first. The
 * choice is kept in `localStorage` for the next visit; every access can fail and the page works without it.
 */
import { useCallback, useState } from 'react'
import type { KeyValueStore } from '@/lib/invites'
import type { PreconListItem } from '@/protocol/generated'

/** How the precon list is ordered. */
export type PreconSort = 'newest' | 'name' | 'favorites'

/** The orders in the menu; `account` marks the one that needs a signed-in player. */
export const PRECON_SORTS: readonly { value: PreconSort; label: string; account?: true }[] = [
  { value: 'newest', label: 'Newest' },
  { value: 'name', label: 'Name' },
  { value: 'favorites', label: 'Favorites first', account: true },
]

/** The name of the list in each order, for the table caption and the list label. */
export const PRECON_SORT_CAPTION: Readonly<Record<PreconSort, string>> = {
  newest: 'Precons, newest release first',
  name: 'Precons by name',
  favorites: 'Precons, your favorites first',
}

/** Whether a value is one of the orders. */
export const isPreconSort = (value: unknown): value is PreconSort =>
  PRECON_SORTS.some((choice) => choice.value === value)

const byName = (a: PreconListItem, b: PreconListItem): number =>
  a.name.localeCompare(b.name, undefined, { sensitivity: 'base', numeric: true }) ||
  a.code.localeCompare(b.code)

/**
 * The precons in the order asked for, without changing the list. Newest keeps the server's order (newest release,
 * then name); favorites first moves the starred precons to the top and keeps that order within both groups.
 */
export function sortPrecons(
  items: readonly PreconListItem[],
  sort: PreconSort,
  favorites: ReadonlySet<string>,
): PreconListItem[] {
  if (sort === 'name') return [...items].sort(byName)
  if (sort === 'favorites') {
    const starred = (item: PreconListItem): number => (favorites.has(item.code) ? 0 : 1)
    return [...items].sort((a, b) => starred(a) - starred(b))
  }
  return [...items]
}

/** The `localStorage` key of the order. */
export const PRECON_SORT_KEY = 'magic.preconSort'

function localStore(): KeyValueStore | null {
  try {
    return window.localStorage
  } catch {
    return null
  }
}

/** The kept order; `newest` when none was kept, the value is no order or the storage cannot be read. */
export function loadPreconSort(store: KeyValueStore | null = localStore()): PreconSort {
  try {
    const kept = store?.getItem(PRECON_SORT_KEY)
    return isPreconSort(kept) ? kept : 'newest'
  } catch {
    return 'newest'
  }
}

/** Keeps the order for the next visit; false when the browser refused. */
export function savePreconSort(sort: PreconSort, store: KeyValueStore | null = localStore()): boolean {
  try {
    if (store === null) return false
    store.setItem(PRECON_SORT_KEY, sort)
    return true
  } catch {
    return false
  }
}

/** The order of the list and the way to change it: the choice shows at once and is kept when the browser allows. */
export function usePreconSort(): [PreconSort, (sort: PreconSort) => void] {
  const [sort, setSort] = useState<PreconSort>(() => loadPreconSort())
  const change = useCallback((next: PreconSort): void => {
    setSort(next)
    savePreconSort(next)
  }, [])
  return [sort, change]
}
