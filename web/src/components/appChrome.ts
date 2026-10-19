import { useLayoutEffect, useSyncExternalStore } from 'react'

// How many pages hold the window at the moment. A page that is the whole window (the game table) takes the header
// away; the count survives a page that swaps with another one within a frame.
let holders = 0
const listeners = new Set<() => void>()

const announce = (): void => {
  for (const listener of listeners) listener()
}

const subscribe = (listener: () => void): (() => void) => {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** Takes the whole window for this page while it is mounted: the shell draws no header and the page never scrolls. */
export function useFullBleed(): void {
  // A layout effect, so the header is gone before the first paint of the page.
  useLayoutEffect(() => {
    holders += 1
    announce()
    return () => {
      holders -= 1
      announce()
    }
  }, [])
}

/** Whether a page holds the whole window. */
export function useIsFullBleed(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => holders > 0,
    () => false,
  )
}
