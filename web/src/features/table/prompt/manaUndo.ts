import type { ManaUndo, ManaUndoRefusal, ObjectId, ObjectView } from '@/protocol/generated'
import type { CardNote, CardNotes } from './marks'

/** Why a source tapped for mana cannot be untapped again, as the card says it. */
export const MANA_UNDO_REASONS: Record<ManaUndoRefusal, string> = {
  Spent: 'That mana was spent',
  Triggered: 'Tapping it caused a trigger',
  DidMore: 'Its ability did more than add mana',
  Since: 'Something happened since',
}

/** The sources a press untaps again (`PriorityOptions.undo_mana` with no refusal). */
export function undoableSources(undo: readonly ManaUndo[] | undefined): ObjectId[] {
  return (undo ?? []).flatMap((entry) => (entry.refusal === undefined ? [entry.source] : []))
}

/**
 * The note of each source tapped for mana since the seat last did anything else: one a press untaps again says so; one
 * that cannot be untapped gives the reason, on its foot while hovered and in its preview.
 */
export function undoNotes(
  undo: readonly ManaUndo[] | undefined,
  objectsById: Readonly<Record<number, ObjectView>>,
): CardNotes {
  const notes = new Map<ObjectId, CardNote>()
  for (const { source, refusal } of undo ?? []) {
    const name = objectsById[source]?.name ?? 'it'
    if (refusal === undefined) {
      notes.set(source, {
        words: `press to untap ${name} and take its mana back`,
        hint: 'Untap',
        line: `Press ${name} again to untap it and take its mana back out of your pool.`,
      })
    } else {
      const reason = MANA_UNDO_REASONS[refusal]
      notes.set(source, {
        words: `cannot untap: ${reason.toLowerCase()}`,
        hint: reason,
        line: `${name} can't be untapped: ${reason.charAt(0).toLowerCase()}${reason.slice(1)}.`,
      })
    }
  }
  return notes
}
