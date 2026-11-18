/**
 * Marks a decision puts on cards of the table without redrawing them: the asking card's glow, the badge of what
 * triggered it, the numbers of triggers being ordered, and the card a chip or a row points at. `AskMarks` draws them
 * as style rules keyed by object id.
 */
import { useSyncExternalStore } from 'react'
import type { ObjectId } from '@/protocol/generated'

/** How a badge looks: the asker's gold, the cause's green, a placed number's rose, a done tick. */
export type BadgeTone = 'ask' | 'cause' | 'placed' | 'done'

/** One card's badge: a short word or number over its top edge. */
export interface BoardBadge {
  id: ObjectId
  label: string
  tone: BadgeTone
}

interface State {
  /** The badges a prompt published (the order of triggers). */
  badges: readonly BoardBadge[]
  /** The card a hovered or pressed chip or row points at. */
  lit: ObjectId | null
}

let state: State = { badges: [], lit: null }
const listeners = new Set<() => void>()

function set(next: State): void {
  state = next
  for (const listener of listeners) listener()
}

const sameBadges = (a: readonly BoardBadge[], b: readonly BoardBadge[]): boolean =>
  a.length === b.length &&
  a.every((one, at) => one.id === b[at]?.id && one.label === b[at]?.label && one.tone === b[at]?.tone)

/** Replaces the badges a prompt puts on cards; an equal list changes nothing. */
export function publishBadges(badges: readonly BoardBadge[]): void {
  if (!sameBadges(state.badges, badges)) set({ ...state, badges })
}

/** Lights `id` on the board (a chip or row is hovered, focused or pressed), or nothing. */
export function lightCard(id: ObjectId | null): void {
  if (state.lit !== id) set({ ...state, lit: id })
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

const read = (): State => state

/** The badges and the lit card, followed. */
export function useBoardBadges(): State {
  return useSyncExternalStore(subscribe, read, read)
}

/** Pointer and focus handlers that light `id` on the board while the element is hovered, focused or pressed. */
export function lightHandlers(id: ObjectId | undefined): {
  onPointerEnter?: () => void
  onPointerLeave?: () => void
  onPointerDown?: () => void
  onPointerUp?: () => void
  onFocus?: () => void
  onBlur?: () => void
} {
  if (id === undefined) return {}
  const on = (): void => lightCard(id)
  const off = (): void => lightCard(null)
  return {
    onPointerEnter: on,
    onPointerLeave: off,
    onPointerDown: on,
    onPointerUp: off,
    onFocus: on,
    onBlur: off,
  }
}
