/**
 * The combat a prompt is still choosing: blocks or attacks that are not sent yet. A prompt publishes them here and
 * the overlay (and everything that must know a forecast is out of date) reads them. Same shape as the marks store in
 * `prompt/marks.ts`: one store per table, provided by `GameTable`, gone when the prompt is.
 */
import { createContext, useContext, useLayoutEffect, useSyncExternalStore } from 'react'
import type { AttackTarget, ObjectId } from '@/protocol/generated'

/** Choices not yet sent. */
export interface PendingCombat {
  /** Blocks being chosen, blocker to attacker, in the order they were made. */
  blocks: ReadonlyMap<ObjectId, ObjectId>
  /** Attacks being chosen, attacker to what it attacks. */
  attacks: ReadonlyMap<ObjectId, AttackTarget>
}

const NO_BLOCKS: ReadonlyMap<ObjectId, ObjectId> = new Map()
const NO_ATTACKS: ReadonlyMap<ObjectId, AttackTarget> = new Map()

/** Nothing chosen. */
export const NO_PENDING: PendingCombat = { blocks: NO_BLOCKS, attacks: NO_ATTACKS }

/** Where prompts leave their unsent choices. */
export interface PendingCombatStore {
  /** The choices now; the same object until they change. */
  get(): PendingCombat
  /** Replaces the choices (a field left out is empty); listeners hear of it only when something differs. */
  set(next: Partial<PendingCombat>): void
  /** Forgets every choice. */
  clear(): void
  /** Calls `listener` after every change; returns the way to stop. */
  subscribe(listener: () => void): () => void
}

/** A store with nothing chosen. */
export function createPendingCombatStore(): PendingCombatStore {
  let current = NO_PENDING
  const listeners = new Set<() => void>()
  const set = (next: Partial<PendingCombat>): void => {
    // An empty map is always the shared one, so "still nothing" is not a change.
    const blocks = next.blocks === undefined || next.blocks.size === 0 ? NO_BLOCKS : next.blocks
    const attacks = next.attacks === undefined || next.attacks.size === 0 ? NO_ATTACKS : next.attacks
    if (blocks === current.blocks && attacks === current.attacks) return
    current = blocks === NO_BLOCKS && attacks === NO_ATTACKS ? NO_PENDING : { blocks, attacks }
    for (const listener of [...listeners]) listener()
  }
  return {
    get: () => current,
    set,
    clear: () => set({}),
    subscribe(listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
  }
}

/** The pending store of the table around the caller; a table-less caller shares one that nothing publishes to. */
export const PendingCombatContext = createContext<PendingCombatStore>(createPendingCombatStore())

/** The pending store of the table around the caller, for a hook that must choose when to listen. */
export function usePendingStore(): PendingCombatStore {
  return useContext(PendingCombatContext)
}

/** The choices a prompt has made and not sent yet; redraws when they change. */
export function usePendingCombat(): PendingCombat {
  const store = usePendingStore()
  return useSyncExternalStore(store.subscribe, store.get)
}

/**
 * Publishes the choices of the calling prompt for as long as it is on screen. Pass memoised maps: a new map every
 * render counts as a change.
 */
export function usePublishPending(next: Partial<PendingCombat>): void {
  const store = usePendingStore()
  const { blocks, attacks } = next
  // Layout effects, so the lines are never drawn once without the choice that was just made.
  useLayoutEffect(() => {
    store.set({ blocks, attacks })
  }, [store, blocks, attacks])
  useLayoutEffect(() => () => store.clear(), [store])
}
