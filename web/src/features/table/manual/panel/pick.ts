/** The card the Commands panel acts on: armed by "Pick a card", chosen by a press on any card of the table, kept until cleared. */
import { createContext, useContext, useSyncExternalStore } from 'react'
import type { ObjectId } from '@/protocol/generated'

/** Where the pick is: nothing, waiting for a press on a card, or a card chosen. */
export type PickState =
  | { phase: 'idle' }
  /** `for` names what the card is wanted for; `was` is the card that stays chosen if the pick is cancelled. */
  | { phase: 'armed'; for: string; was: ObjectId | null }
  | { phase: 'picked'; card: ObjectId }

/** What moves the pick. */
export type PickEvent =
  | { type: 'arm'; for: string }
  | { type: 'choose'; card: ObjectId }
  | { type: 'cancel' }
  | { type: 'clear' }
  | { type: 'gone'; card: ObjectId }

/** The pick before anything happened. */
export const IDLE: PickState = { phase: 'idle' }

/** What "Pick a card" arms for when no row asks for more. */
export const PICK_FOR = 'its commands'

/** The pick after `event`: arming keeps the chosen card for a cancel, a card that leaves the table is forgotten. */
export function nextPick(state: PickState, event: PickEvent): PickState {
  switch (event.type) {
    case 'arm':
      return { phase: 'armed', for: event.for, was: state.phase === 'picked' ? state.card : null }
    case 'choose':
      return { phase: 'picked', card: event.card }
    case 'cancel':
      if (state.phase !== 'armed') return state
      return state.was === null ? IDLE : { phase: 'picked', card: state.was }
    case 'clear':
      return IDLE
    case 'gone':
      if (state.phase === 'picked') return state.card === event.card ? IDLE : state
      if (state.phase === 'armed' && state.was === event.card) return { ...state, was: null }
      return state
  }
}

/** The card the pick holds or held before an arm, if any. */
export function pickedCard(state: PickState): ObjectId | null {
  if (state.phase === 'picked') return state.card
  return state.phase === 'armed' ? state.was : null
}

/** The pick of a table. */
export interface PickStore {
  get(): PickState
  subscribe(listener: () => void): () => void
  dispatch(event: PickEvent): void
  /**
   * Hears where the panel should be: out of the way (false) while a card is waited for, so the table's cards can be
   * pressed, and up (true) once one is chosen or the wait is cancelled. The side area answers it.
   */
  watch(listener: (panelUp: boolean) => void): () => void
}

/** A pick with nothing in it. */
export function createPickStore(): PickStore {
  let state = IDLE
  const listeners = new Set<() => void>()
  const watchers = new Set<(panelUp: boolean) => void>()
  const say = (panelUp: boolean): void => {
    for (const watcher of [...watchers]) watcher(panelUp)
  }
  return {
    get: () => state,
    subscribe(listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
    watch(watcher) {
      watchers.add(watcher)
      return () => {
        watchers.delete(watcher)
      }
    },
    dispatch(event) {
      const next = nextPick(state, event)
      const wasArmed = state.phase === 'armed'
      if (next !== state || event.type === 'arm') {
        state = next
        for (const listener of [...listeners]) listener()
      }
      if (event.type === 'arm') say(false)
      else if (event.type === 'choose' || (event.type === 'cancel' && wasArmed)) say(true)
    },
  }
}

/** The pick of the table around a card or the panel; null outside a table. */
export const PickContext = createContext<PickStore | null>(null)

/** The pick store of the table around the caller, or null. */
export function usePickStore(): PickStore | null {
  return useContext(PickContext)
}

const NEVER = (): (() => void) => () => undefined

/** A value read from the pick; the panel and the banner redraw when it changes. */
export function usePickValue<T>(store: PickStore | null, read: (state: PickState) => T, fallback: T): T {
  return useSyncExternalStore(
    store?.subscribe ?? NEVER,
    () => (store === null ? fallback : read(store.get())),
    () => fallback,
  )
}

/** Whether the table is waiting for a press on a card. */
export function useArmed(): boolean {
  return usePickValue(usePickStore(), (state) => state.phase === 'armed', false)
}

/** The whole pick state; idle outside a table. */
export function usePick(): PickState {
  return usePickValue(usePickStore(), (state) => state, IDLE)
}
