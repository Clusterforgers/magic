/**
 * Who the attack prompt sends creatures at, as the pods show it. The prompt publishes the seats that may be attacked and
 * the one chosen now; a pod subscribes to its own seat only, shows the choice and picks itself when it is clicked. The
 * choice made last is kept after the prompt is gone, so the next attack starts from it. One store per table, provided
 * by `CombatProviders`.
 */
import { createContext, useCallback, useContext, useLayoutEffect, useSyncExternalStore } from 'react'
import type { PlayerId } from '@/protocol/generated'

/** What a pod says of itself while an attack is being declared. */
export type DefenderState = 'candidate' | 'chosen'

/** The seats an attack may go to, the one chosen and what picking a seat does. */
export interface DefenderChoice {
  seats: readonly PlayerId[]
  chosen: PlayerId | null
  onPick(seat: PlayerId): void
}

/** The defenders of the attack being declared. */
export interface DefenderStore {
  /** How a seat's pod reads now, or undefined while it is no choice. */
  stateOf(seat: PlayerId): DefenderState | undefined
  /** Replaces the choice; pods hear of it only when theirs changed. */
  publish(choice: DefenderChoice): void
  /** Forgets the choice (not the last pick). */
  clear(): void
  /** A click on a pod: calls the prompt's handler for a seat that may be attacked. */
  pick(seat: PlayerId): void
  /** The key of the defender picked last (`targetKey`), or null. */
  remembered(): string | null
  /** Keeps `key` as the defender picked last. */
  remember(key: string): void
  /** Calls `listener` when the state of `seat` changes; returns the way to stop. */
  subscribe(seat: PlayerId, listener: () => void): () => void
}

/** A store with no choice. */
export function createDefenderStore(): DefenderStore {
  let current: DefenderChoice | null = null
  let last: string | null = null
  const listeners = new Map<PlayerId, Set<() => void>>()
  const stateOf = (seat: PlayerId): DefenderState | undefined =>
    current === null || !current.seats.includes(seat)
      ? undefined
      : current.chosen === seat
        ? 'chosen'
        : 'candidate'
  const replace = (next: DefenderChoice | null): void => {
    const before = new Map<PlayerId, DefenderState | undefined>()
    for (const seat of [...(current?.seats ?? []), ...(next?.seats ?? [])]) before.set(seat, stateOf(seat))
    current = next
    for (const [seat, state] of before) {
      if (stateOf(seat) !== state) for (const listener of [...(listeners.get(seat) ?? [])]) listener()
    }
  }
  return {
    stateOf,
    publish: replace,
    clear: () => replace(null),
    pick(seat) {
      if (stateOf(seat) !== undefined) current?.onPick(seat)
    },
    remembered: () => last,
    remember(key) {
      last = key
    },
    subscribe(seat, listener) {
      let set = listeners.get(seat)
      if (set === undefined) {
        set = new Set()
        listeners.set(seat, set)
      }
      set.add(listener)
      return () => {
        set.delete(listener)
        if (set.size === 0) listeners.delete(seat)
      }
    },
  }
}

/** Pods outside a table share one store that nothing publishes to. */
export const DefenderContext = createContext<DefenderStore>(createDefenderStore())

/** The defender store of the table around the caller. */
export function useDefenderStore(): DefenderStore {
  return useContext(DefenderContext)
}

/** What a pod says of its seat while an attack is declared, and what a click on it does. */
export function useDefender(seat: PlayerId): { state: DefenderState | undefined; pick(): void } {
  const store = useDefenderStore()
  const subscribe = useCallback((listener: () => void) => store.subscribe(seat, listener), [store, seat])
  const state = useSyncExternalStore(subscribe, () => store.stateOf(seat))
  const pick = useCallback(() => store.pick(seat), [store, seat])
  return { state, pick }
}

/**
 * Publishes the defenders of the calling prompt for as long as it is on screen. `seats` should be memoised; `onPick`
 * may change every render.
 */
export function usePublishDefenders(
  seats: readonly PlayerId[],
  chosen: PlayerId | null,
  onPick: (seat: PlayerId) => void,
): void {
  const store = useDefenderStore()
  // Layout effects, so a pod is never drawn once without the choice that was just made.
  useLayoutEffect(() => {
    store.publish({ seats, chosen, onPick })
  }, [store, seats, chosen, onPick])
  useLayoutEffect(() => () => store.clear(), [store])
}
