/** The moments standing over the table: the store that holds them, ends each on its own timer, and the watch that fills it from the play store. */
import { createContext, useContext } from 'react'
import { createStore } from 'zustand/vanilla'
import type { StoreApi } from 'zustand/vanilla'
import { browserTimers } from '@/net/timers'
import type { Timers } from '@/net/timers'
import type { PlayStore } from '@/play/store'
import { batchBetween } from '../fx/batch'
import { momentMs, momentsOf, withMoments } from './moments'
import type { Moment } from './moments'

/** What stands now. Replaced, never changed. */
export interface MomentsState {
  readonly items: readonly Moment[]
}

/** The moments of one table. */
export interface Moments {
  readonly store: StoreApi<MomentsState>
  /** Puts moments on show, each going by itself after `momentMs`; the oldest go when there are too many. */
  show(moments: readonly Moment[]): void
  /** Ends one moment now (a click, Escape or the Dismiss button). */
  dismiss(key: string): void
  /** Ends every moment now and cancels the timers. */
  clear(): void
}

/** A table's moments with nothing on show; `timers` is the clock the moments end on (the browser's by default). */
export function createMoments(timers: Timers = browserTimers): Moments {
  const store = createStore<MomentsState>(() => ({ items: [] }))
  const waiting = new Map<string, number>()
  const set = (items: readonly Moment[]): void => store.setState({ items })

  function dismiss(key: string): void {
    const timer = waiting.get(key)
    if (timer !== undefined) timers.clearTimeout(timer)
    waiting.delete(key)
    const { items } = store.getState()
    if (items.some((moment) => moment.key === key)) set(items.filter((moment) => moment.key !== key))
  }

  return {
    store,
    show(moments) {
      if (moments.length === 0) return
      const items = withMoments(store.getState().items, moments)
      // A moment pushed out by newer ones keeps no timer.
      for (const [key, timer] of waiting)
        if (!items.some((moment) => moment.key === key)) {
          timers.clearTimeout(timer)
          waiting.delete(key)
        }
      for (const moment of moments)
        if (items.includes(moment))
          waiting.set(
            moment.key,
            timers.setTimeout(() => dismiss(moment.key), momentMs(moment)),
          )
      set(items)
    },
    dismiss,
    clear() {
      for (const timer of waiting.values()) timers.clearTimeout(timer)
      waiting.clear()
      if (store.getState().items.length > 0) set([])
    },
  }
}

/**
 * Puts the moments of every batch the play store applies from now on on show; returns the function that stops
 * watching. A snapshot (a resync, a reconnect) is no batch, so it replays nothing.
 */
export function watchMoments(store: PlayStore, moments: Moments): () => void {
  return store.subscribe((next, prev) => {
    const step = batchBetween(prev, next)
    if (step === null) return
    const nameOf = (id: number): string | undefined =>
      step.after.objectsById[id]?.name ?? step.before.objectsById[id]?.name ?? next.logNames[id]
    moments.show(momentsOf({ entries: step.entries, nameOf }))
  })
}

/** The moments of the table around the caller; outside a table nothing is ever shown. */
export const MomentsContext = createContext<Moments>(createMoments())

/** The moments of the table around the caller. */
export function useMoments(): Moments {
  return useContext(MomentsContext)
}
