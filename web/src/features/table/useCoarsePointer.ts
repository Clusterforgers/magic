import { useSyncExternalStore } from 'react'

const QUERY = '(pointer: coarse)'

// jsdom has no matchMedia; an environment without one has a fine pointer.
const query = (): MediaQueryList | null =>
  typeof window.matchMedia === 'function' ? window.matchMedia(QUERY) : null

function subscribe(onChange: () => void): () => void {
  const list = query()
  list?.addEventListener('change', onChange)
  return () => list?.removeEventListener('change', onChange)
}

/** True while the main pointer is a finger (a touch screen), read now. */
export function hasCoarsePointer(): boolean {
  return query()?.matches ?? false
}

/** True while the main pointer is a finger; follows a change of device. */
export function useCoarsePointer(): boolean {
  return useSyncExternalStore(subscribe, hasCoarsePointer, () => false)
}
