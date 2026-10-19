import { useLayoutEffect, useSyncExternalStore } from 'react'

// How many pages ask for a window of their own at the moment, counted so that a page which swaps with another one
// within a frame does not flicker. A page that is the whole window (the game table) takes the header away; a page
// that fits the window keeps it.
function holders() {
  let count = 0
  const listeners = new Set<() => void>()
  const announce = (): void => {
    for (const listener of listeners) listener()
  }
  const subscribe = (listener: () => void): (() => void) => {
    listeners.add(listener)
    return () => listeners.delete(listener)
  }
  return {
    // A layout effect, so the shell has changed before the first paint of the page.
    useHold(): void {
      useLayoutEffect(() => {
        count += 1
        announce()
        return () => {
          count -= 1
          announce()
        }
      }, [])
    },
    useHeld(): boolean {
      return useSyncExternalStore(
        subscribe,
        () => count > 0,
        () => false,
      )
    },
  }
}

const bleed = holders()
const fit = holders()

/** Takes the whole window for this page while it is mounted: the shell draws no header and the page never scrolls. */
export const useFullBleed = bleed.useHold

/** Whether a page holds the whole window. */
export const useIsFullBleed = bleed.useHeld

/**
 * Fits this page to the window under the header while it is mounted: the shell is exactly the window high and `main`
 * is the rest, so the page can scroll one region inside itself instead of the page.
 */
export const useFitWindow = fit.useHold

/** Whether a page fits the window under the header. */
export const useIsFitWindow = fit.useHeld
