/**
 * Whether the window was resized a moment ago: the table's regions settle over a few frames after a resize, and a board
 * fits afresh meanwhile instead of holding a size from a frame in between (`fitHeld`).
 */

/** How long after a window resize a board fits afresh, in ms. */
export const RESIZE_SETTLE_MS = 800

let resizedAt = Number.NEGATIVE_INFINITY

/** Notes a resize at `now`; the window's resize listener calls it. */
export function noteResize(now: number): void {
  resizedAt = now
}

/** Whether `now` is within `RESIZE_SETTLE_MS` of the last resize. */
export function settling(now: number): boolean {
  return now - resizedAt < RESIZE_SETTLE_MS
}

if (typeof window !== 'undefined') {
  window.addEventListener('resize', () => noteResize(performance.now()))
}
