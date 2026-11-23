/**
 * What a drag of a card of the hand dock means (docs/web.md, Hand dock): moved along the hand it reorders, carried up
 * out of the dock onto the table it plays the card (a land played, a spell cast, never an ability of the card), and let
 * go anywhere else it does nothing. Pure, but for `dropCard`, which hands the card to the prompt's drop handler. Nothing
 * here talks to the server: a drag is the viewer's own until the drop.
 */
import type { ObjectId } from '@/protocol/generated'
import type { MarksStore } from './prompt/marks'

/** How far up a card is carried, in px, before the drag means "play"; a drag along the hand strays less. */
export const PLAY_RISE_PX = 40

/** Where the pointer of a drag is: still over the hand dock, or up on the table. */
export type DragZone = 'hand' | 'board'

/**
 * The zone of a pointer at `y` that went down at `startY`, both in viewport px, over a dock whose top edge is at
 * `dockTop`: on the table once it is above the dock and has risen `PLAY_RISE_PX`, so neither a lifted card grabbed by
 * its top nor a move between the rows of a tall hand counts.
 */
export function dragZone(startY: number, y: number, dockTop: number): DragZone {
  return y < dockTop && startY - y >= PLAY_RISE_PX ? 'board' : 'hand'
}

/** What a drop does: play the card, move it from one place of the hand to another, or nothing. */
export type DragOutcome =
  { kind: 'play' } | { kind: 'reorder'; from: number; to: number } | { kind: 'cancel' }

/** The facts of a drop. */
export interface Drop {
  zone: DragZone
  /** A drop on the table plays the card now (`playableBy`). */
  playable: boolean
  /** The hand takes a new order (the cards at the head of the dock do not). */
  sortable: boolean
  /** The place of the dragged card, and of the card it was let go over; -1 for none. */
  from: number
  to: number
}

/**
 * What a drop means: on the table a playable card plays and any other returns to the hand; over the hand the card
 * moves to the place it was let go over, when the hand can be ordered and that is another place.
 */
export function dragOutcome({ zone, playable, sortable, from, to }: Drop): DragOutcome {
  if (zone === 'board') return playable ? { kind: 'play' } : { kind: 'cancel' }
  if (sortable && from >= 0 && to >= 0 && from !== to) return { kind: 'reorder', from, to }
  return { kind: 'cancel' }
}

/**
 * True when the prompt on screen plays the card on a drop (`MarksStore.drops`): a land to play or a spell to cast. A
 * card that only has an ability to activate (cycling with a cast it cannot pay) and every card during a pick among the
 * hand (a discard, a payment) is not.
 */
export function playableBy(marks: Pick<MarksStore, 'drops'>, id: ObjectId): boolean {
  return marks.drops(id)
}

/**
 * Plays a card dropped on the table through the prompt's drop handler (`MarksStore.drop` with the card's element),
 * which plays the land or casts the spell with the answer a click sends for that. False when the card cannot be played
 * now; nothing is called then.
 */
export function dropCard(
  marks: Pick<MarksStore, 'drops' | 'drop'>,
  id: ObjectId,
  element: HTMLElement | undefined,
): boolean {
  if (!playableBy(marks, id)) return false
  marks.drop(id, element)
  return true
}

/** The viewport y a mouse or touch event happened at, or null for another event. */
export function eventY(event: Event | null): number | null {
  if (event === null) return null
  if ('clientY' in event && typeof event.clientY === 'number') return event.clientY
  if ('touches' in event) {
    const touches = event.touches as ArrayLike<{ clientY: number }>
    return touches[0]?.clientY ?? null
  }
  return null
}
