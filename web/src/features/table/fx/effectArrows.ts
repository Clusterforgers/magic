/**
 * The short arrows from a resolving object to what it counters, exiles, bounces or destroys, playing right now
 * (docs/ui-design.md, Playing is legible). The ends are measured when the batch arrives, before React draws it, since
 * the object and its victim may already be gone from the table; the combat overlay draws the lines. Like the rest of
 * the decoration layer it holds no game state and nothing waits on it, and it stays under reduced motion.
 */
import { createStore } from 'zustand/vanilla'
import type { StoreApi } from 'zustand/vanilla'
import type { Timers } from '@/net/timers'
import type { MarkArrow } from './marks'
import { MOVE_MS } from './moves'
import { measureObject, measurePlace } from './places'
import type { Box } from './places'

/** How long an arrow stays drawn, in milliseconds (times the dev slow-down): it draws in, then holds for the mark. */
export const EFFECT_ARROW_MS = MOVE_MS.arrow + MOVE_MS.mark

/** One arrow while it plays; `token` is new for every arrow, so a repeat restarts. */
export interface EffectArrow {
  readonly token: number
  readonly kind: MarkArrow['kind']
  readonly from: Box
  readonly to: Box
}

/** No arrows; one value, so a table with nothing playing keeps the same snapshot. */
export const NO_EFFECT_ARROWS: readonly EffectArrow[] = []

/** What `createEffectArrows` needs from the decoration layer around it. */
export interface EffectArrowsOptions {
  timers: Timers
  /** Whether the layer plays at all now; the arrow plays in `reduced` too. */
  enabled: () => boolean
  slow: number
  /** What to look for objects in; the document by default. */
  root?: ParentNode | undefined
}

/** The effect arrows of one table. */
export interface EffectArrows {
  readonly store: StoreApi<readonly EffectArrow[]>
  /** Measures each arrow's ends now and draws the ones whose ends are on screen; they end on their own. */
  play(arrows: readonly MarkArrow[]): void
  /** Ends every arrow now and cancels the timers. */
  clear(): void
}

/** An arrow layer with nothing playing. */
export function createEffectArrows({ timers, enabled, slow, root }: EffectArrowsOptions): EffectArrows {
  const store = createStore<readonly EffectArrow[]>(() => NO_EFFECT_ARROWS)
  const waiting = new Set<number>()
  let last = 0
  return {
    store,
    play(arrows) {
      if (!enabled() || arrows.length === 0) return
      const page = root ?? document
      const started: EffectArrow[] = []
      for (const { by, object, kind } of arrows) {
        const to = measureObject(page, object)
        // A resolving object that was cast in this very batch has no chip yet: the stack stands for it.
        const from = measureObject(page, by) ?? measurePlace(page, { stack: true })
        if (to !== null && from !== null) started.push({ token: (last += 1), kind, from: from.box, to: to.box })
      }
      if (started.length === 0) return
      const tokens = new Set(started.map((arrow) => arrow.token))
      store.setState([...store.getState(), ...started], true)
      const timer = timers.setTimeout(() => {
        waiting.delete(timer)
        const kept = store.getState().filter((arrow) => !tokens.has(arrow.token))
        store.setState(kept.length === 0 ? NO_EFFECT_ARROWS : kept, true)
      }, EFFECT_ARROW_MS * slow)
      waiting.add(timer)
    },
    clear() {
      for (const timer of waiting) timers.clearTimeout(timer)
      waiting.clear()
      if (store.getState().length > 0) store.setState(NO_EFFECT_ARROWS, true)
    },
  }
}
