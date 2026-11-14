/**
 * Whether the viewer folded the side column (Commands, Log, Chat, Stack) to its icon rail, kept in this browser's
 * `localStorage`; open until folded. Only a window wide enough for a column (L and up) reads it.
 */
import { useSyncExternalStore } from 'react'

/** The `localStorage` key of the fold: `rail` while folded. */
export const SIDE_FOLD_KEY = 'mtg.side'

const listeners = new Set<() => void>()
// The fold of this page where storage is blocked, so the button still answers.
let fallback = false

/** Whether the column is folded to its rail, read now. */
export function sideFolded(): boolean {
  try {
    return localStorage.getItem(SIDE_FOLD_KEY) === 'rail'
  } catch {
    return fallback
  }
}

/** Folds the column to its rail or opens it again, for every game in this browser. */
export function setSideFolded(folded: boolean): void {
  fallback = folded
  try {
    if (folded) localStorage.setItem(SIDE_FOLD_KEY, 'rail')
    else localStorage.removeItem(SIDE_FOLD_KEY)
  } catch {
    // Blocked storage: the fold lasts for this page only.
  }
  for (const listener of listeners) listener()
}

function subscribe(onChange: () => void): () => void {
  listeners.add(onChange)
  const onStorage = (event: StorageEvent) => {
    if (event.key === SIDE_FOLD_KEY) onChange()
  }
  window.addEventListener('storage', onStorage)
  return () => {
    listeners.delete(onChange)
    window.removeEventListener('storage', onStorage)
  }
}

/** Whether the column is folded, followed across tabs. */
export function useSideFolded(): boolean {
  return useSyncExternalStore(subscribe, sideFolded, () => false)
}
