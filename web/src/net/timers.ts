/** The clock and timer functions the network layer uses; tests pass a manual clock instead. */
export interface Timers {
  /** Runs `callback` once after `ms` milliseconds and returns an id for `clearTimeout`. */
  setTimeout(callback: () => void, ms: number): number
  /** Cancels a timer that has not fired yet. */
  clearTimeout(id: number): void
  /** Milliseconds on a clock that only moves forward. */
  now(): number
}

/** The browser's timers. */
export const browserTimers: Timers = {
  setTimeout: (callback, ms) => window.setTimeout(callback, ms),
  clearTimeout: (id) => window.clearTimeout(id),
  now: () => performance.now(),
}
