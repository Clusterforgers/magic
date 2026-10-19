import { useCallback, useSyncExternalStore } from 'react'

/** True while the window matches the media query; follows a change. `fallback` stands in where there is no `matchMedia`. */
export function useMediaQuery(query: string, fallback: boolean): boolean {
  // jsdom has no matchMedia; an environment without one gets the fallback.
  const list = useCallback(
    (): MediaQueryList | null => (typeof window.matchMedia === 'function' ? window.matchMedia(query) : null),
    [query],
  )
  const subscribe = useCallback(
    (onChange: () => void): (() => void) => {
      const found = list()
      found?.addEventListener('change', onChange)
      return () => found?.removeEventListener('change', onChange)
    },
    [list],
  )
  return useSyncExternalStore(
    subscribe,
    () => list()?.matches ?? fallback,
    () => fallback,
  )
}
