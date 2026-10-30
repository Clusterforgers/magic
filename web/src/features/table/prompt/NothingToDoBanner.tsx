import { NOTE_TEXT, useNothingToDo } from './nothingToDo'
import { useRewound } from './rewound'

/**
 * Says in plain words that the player has no attackers or blockers, or that a play they began was taken back and why.
 * The live region is always in the page and only its content comes and goes, so a screen reader announces the note.
 * It has no `role="status"`: the connection notice is the page's one status.
 */
export function NothingToDoBanner() {
  const note = useNothingToDo()
  const rewound = useRewound()
  return (
    <div aria-live="polite" data-slot="nothing-to-do-region" className="basis-full empty:hidden">
      {rewound === null ? null : (
        <p data-slot="rewound" className="w-fit rounded-md bg-accent px-2 py-1 text-sm text-accent-foreground">
          {rewound.text}
        </p>
      )}
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
