import { useSyncExternalStore } from 'react'

const QUERY = '(prefers-reduced-motion: reduce)'

// jsdom has no matchMedia; an environment without one counts as "motion is fine".
const query = (): MediaQueryList | null =>
  typeof window.matchMedia === 'function' ? window.matchMedia(QUERY) : null

function subscribe(onChange: () => void): () => void {
  const list = query()
  list?.addEventListener('change', onChange)
  return () => list?.removeEventListener('change', onChange)
}

/** True while the person asks their system for less motion; follows a change of the setting. */
export function useReducedMotion(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => query()?.matches ?? false,
    () => false,
  )
}
