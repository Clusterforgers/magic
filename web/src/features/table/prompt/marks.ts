/**
 * How the active prompt marks cards. A prompt publishes a mark per card id and what a click on a marked card does;
 * each `Card` subscribes to its own id only, so publishing redraws exactly the cards whose mark changed.
 */
import { createContext, useCallback, useContext, useLayoutEffect, useRef, useSyncExternalStore } from 'react'
import type { ObjectId } from '@/protocol/generated'
import type { Highlight } from '../cardModel'

/** The mark of every card a prompt cares about; a card that is not in the map is not marked. */
export type CardMarks = ReadonlyMap<ObjectId, Highlight>

/** A map with no marks. */
export const NO_MARKS: CardMarks = new Map()

/**
 * What a prompt says on a card: `words` for a screen reader, a short `label` drawn on the face only where the state has
 * no arrow or ring of its own (an attacker that still needs blockers), and `cue` for the card a click flow waits on.
 */
export interface CardNote {
  label?: string
  words: string
  cue?: 'active'
}

/** The note of every card a prompt writes on, such as "Needs 2 more" on a menace attacker. */
export type CardNotes = ReadonlyMap<ObjectId, CardNote>

/** A map with no notes. */
export const NO_NOTES: CardNotes = new Map()

const sameNote = (a: CardNote, b: CardNote): boolean =>
  a.label === b.label && a.words === b.words && a.cue === b.cue

/** Every card of `own` dimmed, then the `legal` ones legal and the `selected` ones selected (they may lie outside `own`). */
export function markCards(
  own: readonly ObjectId[],
  legal: Iterable<ObjectId>,
  selected: Iterable<ObjectId> = [],
): CardMarks {
  const marks = new Map<ObjectId, Highlight>()
  for (const id of own) marks.set(id, 'dimmed')
  for (const id of legal) marks.set(id, 'legal')
  for (const id of selected) marks.set(id, 'selected')
  return marks
}

/** What a prompt tells the cards. Read it through `useCardMark`, write it through `useCardMarks`. */
export interface MarksStore {
  /** The mark of one card, or undefined. */
  markOf(id: ObjectId): Highlight | undefined
  /** True when marked cards are toggles (a selection), false when a click acts at once. */
  toggles(): boolean
  /** Calls `listener` when the mark of `id` changes, or when `toggles` does. */
  subscribe(id: ObjectId, listener: () => void): () => void
  /** Replaces every mark and the click handler; only cards whose mark changed are notified. */
  publish(marks: CardMarks, onSelect: ((id: ObjectId) => void) | undefined, toggles: boolean): void
  /** Removes every mark. */
  clear(): void
  /** The note written on one card, or undefined. */
  noteOf(id: ObjectId): CardNote | undefined
  /** Replaces every note; only cards whose note changed are notified. */
  publishNotes(notes: CardNotes): void
  /** Removes every note. */
  clearNotes(): void
  /** A click or key press on a marked card; goes to the published handler. */
  select(id: ObjectId): void
}

/** A store with no marks. */
export function createMarksStore(): MarksStore {
  let marks: CardMarks = NO_MARKS
  let handler: ((id: ObjectId) => void) | undefined
  let toggling = false
  let notes: CardNotes = NO_NOTES
  const listeners = new Map<ObjectId, Set<() => void>>()
  const notify = (ids: Iterable<ObjectId>): void => {
    for (const id of ids) for (const listener of [...(listeners.get(id) ?? [])]) listener()
  }
  const replace = (next: CardMarks, nextToggles: boolean): void => {
    const before = marks
    const flipped = nextToggles !== toggling
    marks = next
    toggling = nextToggles
    if (flipped) {
      // Only a marked card reads differently as a toggle, so a table of hundreds of cards is not redrawn for it.
      notify(new Set([...before.keys(), ...next.keys()]))
      return
    }
    const changed: ObjectId[] = []
    for (const [id, mark] of next) if (before.get(id) !== mark) changed.push(id)
    for (const id of before.keys()) if (!next.has(id)) changed.push(id)
    notify(changed)
  }
  const replaceNotes = (next: CardNotes): void => {
    const before = notes
    // A note that reads the same keeps its object, so a card's snapshot stays the same value.
    const kept = new Map<ObjectId, CardNote>()
    const changed: ObjectId[] = []
    for (const [id, note] of next) {
      const old = before.get(id)
      if (old !== undefined && sameNote(old, note)) kept.set(id, old)
      else {
        kept.set(id, note)
        changed.push(id)
      }
    }
    for (const id of before.keys()) if (!next.has(id)) changed.push(id)
    notes = kept
    notify(changed)
  }
  return {
    markOf: (id) => marks.get(id),
    toggles: () => toggling,
    subscribe(id, listener) {
      let set = listeners.get(id)
      if (set === undefined) {
        set = new Set()
        listeners.set(id, set)
      }
      set.add(listener)
      return () => {
        set.delete(listener)
        if (set.size === 0) listeners.delete(id)
      }
    },
    publish(next, onSelect, toggles) {
      handler = onSelect
      replace(next, toggles)
    },
    clear() {
      handler = undefined
      replace(NO_MARKS, false)
    },
    select: (id) => handler?.(id),
    noteOf: (id) => notes.get(id),
    publishNotes: replaceNotes,
    clearNotes: () => replaceNotes(NO_NOTES),
  }
}

/** Cards outside a table share this one, which nothing publishes to. */
export const MarksContext = createContext<MarksStore>(createMarksStore())

/** The marks store of the table around the caller. */
export function useMarksStore(): MarksStore {
  return useContext(MarksContext)
}

/** What the active prompt says about one card: its mark, whether marked cards are toggles, and its note. */
export function useCardMark(id: ObjectId): {
  highlight: Highlight | undefined
  toggles: boolean
  note: CardNote | undefined
} {
  const store = useMarksStore()
  const subscribe = useCallback((listener: () => void) => store.subscribe(id, listener), [store, id])
  const highlight = useSyncExternalStore(subscribe, () => store.markOf(id))
  const toggles = useSyncExternalStore(subscribe, store.toggles)
  const note = useSyncExternalStore(subscribe, () => store.noteOf(id))
  return { highlight, toggles, note }
}

/** Options of `useCardMarks`. */
export interface CardMarksOptions {
  /** Marked cards are toggles that read as pressed or not (a selection); the default is a click that acts at once. */
  toggles?: boolean
}

/**
 * Marks cards for as long as the calling prompt is on screen. `marks` should be memoised (a new map every render is
 * harmless but wasteful); `onSelect` is called with the id of a legal or selected card that is clicked or activated
 * by keyboard, and it may change every render.
 */
export function useCardMarks(
  marks: CardMarks,
  onSelect?: (id: ObjectId) => void,
  { toggles = false }: CardMarksOptions = {},
): void {
  const store = useMarksStore()
  const latest = useRef(onSelect)
  // Layout effects, so a card is never drawn once with the previous prompt's marks.
  useLayoutEffect(() => {
    latest.current = onSelect
  })
  const forward = useCallback((id: ObjectId) => latest.current?.(id), [])
  useLayoutEffect(() => {
    store.publish(marks, forward, toggles)
  }, [store, marks, forward, toggles])
  useLayoutEffect(() => () => store.clear(), [store])
}

/**
 * Writes a note on cards for as long as the calling prompt is on screen (a declared attacker says whom it attacks,
 * a blocker whom it blocks). Notes only inform: they are not clickable and do not change a card's mark.
 */
export function useCardNotes(notes: CardNotes): void {
  const store = useMarksStore()
  useLayoutEffect(() => {
    store.publishNotes(notes)
  }, [store, notes])
  useLayoutEffect(() => () => store.clearNotes(), [store])
}
