/** Whether the table plays its sound cues, kept in this browser's `localStorage`; on until muted. */
import { useSyncExternalStore } from 'react'

/** The `localStorage` key of the mute: `off` while muted. */
export const SOUNDS_KEY = 'mtg.sounds'

const listeners = new Set<() => void>()

/** Whether sound cues play, read now. */
export function soundsOn(): boolean {
  try {
    return localStorage.getItem(SOUNDS_KEY) !== 'off'
  } catch {
    return true
  }
}

/** Turns the cues on or off in this browser, for every game and page. */
export function setSoundsOn(on: boolean): void {
  try {
    if (on) localStorage.removeItem(SOUNDS_KEY)
    else localStorage.setItem(SOUNDS_KEY, 'off')
  } catch {
    // Storage that is blocked keeps the default; the switch still reads it back.
  }
  for (const listener of listeners) listener()
}

function subscribe(onChange: () => void): () => void {
  listeners.add(onChange)
  const onStorage = (event: StorageEvent) => {
    if (event.key === SOUNDS_KEY) onChange()
  }
  window.addEventListener('storage', onStorage)
  return () => {
    listeners.delete(onChange)
    window.removeEventListener('storage', onStorage)
  }
}

/** Whether sound cues play; follows a change made here or in another tab. */
export function useSoundsOn(): boolean {
  return useSyncExternalStore(subscribe, soundsOn, () => true)
}
