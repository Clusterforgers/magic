/**
 * Whether the boards lay their lands out or fold them into their pods' lands chips, the viewer's "Show lands" choice
 * kept in this browser's `localStorage`. Until the viewer chooses, a desktop table shows lands and a phone folds them.
 */
import { createContext, useSyncExternalStore } from 'react'

/** The `localStorage` key of the choice: `show` or `fold`; none until chosen. */
export const LANDS_KEY = 'mtg.lands'

/** The viewer's choice; null while they have made none. */
export type LandsChoice = 'show' | 'fold' | null

const listeners = new Set<() => void>()
// The choice of this page where storage is blocked, so the switch still answers.
let fallback: LandsChoice = null

/** The kept choice, read now. */
export function landsChoice(): LandsChoice {
  try {
    const kept = localStorage.getItem(LANDS_KEY)
    return kept === 'show' || kept === 'fold' ? kept : null
  } catch {
    return fallback
  }
}

/** Whether lands are laid out: the choice, else shown on a desktop and folded on a phone. */
export const landsShownBy = (choice: LandsChoice, phone: boolean): boolean =>
  choice === null ? !phone : choice === 'show'

/** Lays the lands of every board out or folds them into the chips, for every game in this browser. */
export function setLandsShown(shown: boolean): void {
  fallback = shown ? 'show' : 'fold'
  try {
    localStorage.setItem(LANDS_KEY, fallback)
  } catch {
    // Blocked storage: the choice lasts for this page only.
  }
  for (const listener of listeners) listener()
}

function subscribe(onChange: () => void): () => void {
  listeners.add(onChange)
  const onStorage = (event: StorageEvent) => {
    if (event.key === LANDS_KEY || event.key === null) onChange()
  }
  window.addEventListener('storage', onStorage)
  return () => {
    listeners.delete(onChange)
    window.removeEventListener('storage', onStorage)
  }
}

/** Whether the boards lay their lands out on this kind of window, followed across tabs. */
export function useLandsShown(phone: boolean): boolean {
  const choice = useSyncExternalStore(subscribe, landsChoice, () => null)
  return landsShownBy(choice, phone)
}

/** What the table menu and every board read: whether lands are laid out now; provided by `GameTable`. */
export const LandsShownContext = createContext(true)
