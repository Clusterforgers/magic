/**
 * The order the deck chooser's lists last used, kept in `localStorage` so the next lobby opens with it. Every access can
 * fail (blocked storage, private windows, a full quota) and the chooser works without it.
 */
import { useCallback, useState } from 'react'
import type { KeyValueStore } from '@/lib/invites'
import { isSortKey } from './deckEntries'
import type { SortKey } from './deckEntries'

/** The `localStorage` key of the order. */
export const SORT_KEY = 'magic.deckSort'

function localStore(): KeyValueStore | null {
  try {
    return window.localStorage
  } catch {
    return null
  }
}

/** The remembered order; the default one when none was kept, the kept value is no order or the storage cannot be read. */
export function loadSort(store: KeyValueStore | null = localStore()): SortKey {
  try {
    const kept = store?.getItem(SORT_KEY)
    return isSortKey(kept) ? kept : 'default'
  } catch {
    return 'default'
  }
}

/** Keeps `sort` for the next lobby; false when the browser refused. */
export function saveSort(sort: SortKey, store: KeyValueStore | null = localStore()): boolean {
  try {
    if (store === null) return false
    store.setItem(SORT_KEY, sort)
    return true
  } catch {
    return false
  }
}

/** The order of the lists and the way to change it: the choice shows at once and is kept when the browser allows. */
export function useDeckSort(): [SortKey, (sort: SortKey) => void] {
  const [sort, setSort] = useState<SortKey>(() => loadSort())
  const change = useCallback((next: SortKey): void => {
    setSort(next)
    saveSort(next)
  }, [])
  return [sort, change]
}
