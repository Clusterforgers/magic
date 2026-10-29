/** What a card with clauses left to the table asks of the page: the clauses, the key that lists them and how it opens the list. */
import type { KeyboardEvent } from 'react'
import type { ManualClause, ObjectId, ObjectView } from '@/protocol/generated'
import type { MenuStore } from './menuStore'

/** The clauses of a card the engine does not run; none for a card it runs whole. */
export const clausesOf = (object: ObjectView): readonly ManualClause[] => object.manual ?? []

/** The key that lists the clauses on a focused card. */
export const BY_HAND_KEY = 'h'

/** Whether a key press asks for the list: the plain key, with no modifier. */
export function opensClauses(
  event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'altKey' | 'metaKey' | 'shiftKey'>,
): boolean {
  if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) return false
  return event.key.toLowerCase() === BY_HAND_KEY
}

/** Opens the list of a card's clauses below `at`; `opener` gets the focus back when it closes. */
export function showClauses(store: MenuStore, id: ObjectId, at: HTMLElement, opener: HTMLElement | null): void {
  const { left, bottom } = at.getBoundingClientRect()
  store.open({ id, kind: 'clauses', x: left, y: bottom, opener })
}
