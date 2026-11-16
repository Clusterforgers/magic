import { NOTE_SHORT, NOTE_TEXT, useNothingToDo } from './nothingToDo'
import { useRewound } from './rewound'

/**
 * Says in plain words that the player has no attackers or blockers, or that a play they began was taken back and why.
 * The live region is always in the page and only its content comes and goes, so a screen reader announces the note.
 * It has no `role="status"`: the connection notice is the page's one status. `beside` a prompt the note is one short
 * line of the band's status, as high as the lines under it; a reader and the tooltip still get the whole sentence.
 */
export function NothingToDoBanner({ beside = false }: { beside?: boolean }) {
  const note = useNothingToDo()
  const rewound = useRewound()
  const look = beside
    ? 'w-fit rounded-sm bg-accent px-1.5 text-xs leading-4 font-medium text-accent-foreground'
    : 'w-fit rounded-md bg-accent px-2 py-1 text-sm text-accent-foreground'
  return (
    <div aria-live="polite" data-slot="nothing-to-do-region" className="basis-full empty:hidden">
      {rewound === null ? null : (
        <p data-slot="rewound" title={beside ? rewound.text : undefined} className={look}>
          {rewound.text}
        </p>
      )}
      {note === null ? null : (
        <p
          data-slot="nothing-to-do"
          data-kind={note.kind}
          title={beside ? NOTE_TEXT[note.kind] : undefined}
          className={look}
        >
          {beside ? (
            <>
              <span aria-hidden="true">{NOTE_SHORT[note.kind]}</span>
              <span className="sr-only">{NOTE_TEXT[note.kind]}</span>
            </>
          ) : (
            NOTE_TEXT[note.kind]
          )}
        </p>
      )}
    </div>
  )
}
