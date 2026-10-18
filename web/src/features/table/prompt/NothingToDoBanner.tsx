import { NOTE_TEXT, useNothingToDo } from './nothingToDo'

/**
 * Says in plain words that the player has no attackers or blockers. The status region is always in the page and
 * only its content comes and goes, so a screen reader announces the note.
 */
export function NothingToDoBanner() {
  const note = useNothingToDo()
  return (
    <div role="status" data-slot="nothing-to-do-region" className="basis-full empty:hidden">
      {note === null ? null : (
        <p
          data-slot="nothing-to-do"
          data-kind={note.kind}
          className="w-fit rounded-md bg-accent px-2 py-1 text-sm text-accent-foreground"
        >
          {NOTE_TEXT[note.kind]}
        </p>
      )}
    </div>
  )
}
