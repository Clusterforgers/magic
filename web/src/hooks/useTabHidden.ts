import { useSyncExternalStore } from 'react'

function subscribe(onChange: () => void): () => void {
  document.addEventListener('visibilitychange', onChange)
  return () => document.removeEventListener('visibilitychange', onChange)
}

/** True while the browser tab is in the background (another tab or a minimised window). */
export function useTabHidden(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => document.visibilityState === 'hidden',
    () => false,
  )
}
