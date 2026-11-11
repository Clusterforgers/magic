import { createStore } from 'zustand/vanilla'
import { TOUR } from './tourSteps'

/** The `localStorage` key that says the tour was seen in this browser. */
export const TOUR_SEEN_KEY = 'table-tour'

/** The step on show, or null while the tour is shut. */
export const tourStore = createStore<{ step: number | null }>(() => ({ step: null }))

/** Whether the tour was finished or skipped in this browser; true where storage is missing, so it never nags. */
export function tourSeen(): boolean {
  try {
    return window.localStorage.getItem(TOUR_SEEN_KEY) !== null
  } catch {
    return true
  }
}

function markSeen(): void {
  try {
    window.localStorage.setItem(TOUR_SEEN_KEY, 'seen')
  } catch {
    // Storage refused: the tour may come again next time, which is harmless.
  }
}

/** Starts the tour at its first step. */
export function startTour(): void {
  tourStore.setState({ step: 0 })
}

/** Shuts the tour and remembers it was seen. */
export function endTour(): void {
  markSeen()
  tourStore.setState({ step: null })
}

/** Moves `by` steps; past the last one the tour ends, before the first it stays. */
export function moveTour(by: 1 | -1): void {
  const { step } = tourStore.getState()
  if (step === null) return
  const next = step + by
  if (next >= TOUR.length) endTour()
  else tourStore.setState({ step: Math.max(0, next) })
}
