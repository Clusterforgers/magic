/**
 * The arrows of attachments that moved, playing right now: one from the old host to the new one (docs/ui-design.md,
 * Attachments). Like the rest of the decoration layer it holds no game state and nothing waits on it; unlike the
 * motion effects it stays under reduced motion, where the arrow is drawn still.
 */
import { createStore } from 'zustand/vanilla'
import type { StoreApi } from 'zustand/vanilla'
import type { Timers } from '@/net/timers'
import type { AttachChange } from './derive'

/** How long an arrow stays drawn, in milliseconds (times the dev slow-down); `combat.css` draws it in within this. */
export const ARROW_MS = 1000

/** The most arrows one batch draws; a mass re-equip stays readable. */
export const MAX_ARROWS = 4

/** One arrow while it plays; `token` is new for every arrow, so a repeat restarts. */
export interface AttachArrow {
  readonly token: number
  readonly change: AttachChange
}

/** No arrows; one value, so a table with nothing playing keeps the same snapshot. */
export const NO_ARROWS: readonly AttachArrow[] = []

/** What `createArrows` needs from the decoration layer around it. */
export interface ArrowsOptions {
  timers: Timers
  /** Whether the layer plays at all now; the arrow plays in `reduced` too. */
  enabled: () => boolean
  slow: number
}

/** The arrows of one table. */
export interface Arrows {
  readonly store: StoreApi<readonly AttachArrow[]>
  /** Draws an arrow for each change that has a new host, each ending on its own after `ARROW_MS`. */
  play(changes: readonly AttachChange[]): void
  /** Ends every arrow now and cancels the timers. */
  clear(): void
}

/** An arrow layer with nothing playing. */
export function createArrows({ timers, enabled, slow }: ArrowsOptions): Arrows {
  const store = createStore<readonly AttachArrow[]>(() => NO_ARROWS)
  const waiting = new Set<number>()
  let last = 0
  return {
    store,
    play(changes) {
      if (!enabled()) return
      const started = changes
        .filter((change) => change.to !== null)
        .slice(0, MAX_ARROWS)
        .map((change): AttachArrow => ({ token: (last += 1), change }))
      if (started.length === 0) return
      const tokens = new Set(started.map((arrow) => arrow.token))
      store.setState([...store.getState(), ...started], true)
      const timer = timers.setTimeout(() => {
        waiting.delete(timer)
        const kept = store.getState().filter((arrow) => !tokens.has(arrow.token))
        store.setState(kept.length === 0 ? NO_ARROWS : kept, true)
      }, ARROW_MS * slow)
      waiting.add(timer)
    },
    clear() {
      for (const timer of waiting) timers.clearTimeout(timer)
      waiting.clear()
      if (store.getState().length > 0) store.setState(NO_ARROWS, true)
    },
  }
}
