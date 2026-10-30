/** The small marks of what Reveal and Note put in the view, on a card's face: "Revealed" and the first words of a note. */
import { Eye, StickyNote } from 'lucide-react'
import { cn } from '@/lib/utils'
import { noteLead, noteName, REVEALED_WORD } from './revealed'

/** The width in px of the foot stack from which "Revealed" shows its word beside the icon. */
export const REVEALED_WORD_PX = 72

/** The width in px of the foot stack from which a note shows its first words beside the icon. */
export const NOTE_TEXT_PX = 56

const MARK =
  'flex h-fit max-w-full items-center gap-0.5 rounded-sm px-1 leading-4 font-semibold whitespace-nowrap shadow-sm'

interface MarkProps {
  /** The icon alone: a compact face has no room for the words. */
  compact: boolean
  /** The text size of the face (`SIZE[size].text`). */
  text: string
}

/** "Revealed" with an eye: the card is face up to every seat; the icon alone on a compact face, with the word in its name. */
export function RevealedMark({ compact, text }: MarkProps) {
  return (
    <span
      role="img"
      aria-label={`${REVEALED_WORD} to every seat`}
      title={`${REVEALED_WORD} to every seat`}
      data-slot="card-revealed"
      className={cn(MARK, 'border border-foreground bg-background text-foreground', text)}
    >
      <Eye aria-hidden="true" className="size-3 shrink-0" />
      {compact ? null : <span className="truncate">{REVEALED_WORD}</span>}
    </span>
  )
}

interface HandMarksProps {
  revealed: boolean
  note: string | undefined
  /** A compact face: icons alone. */
  small: boolean
  /** The width in px of the foot stack the marks sit in. */
  footWidth: number
  text: string
}

/** The marks of what the table put on a card, for the foot of a picture: "Revealed" and the note, each with its word where the foot has room. */
export function HandMarks({ revealed, note, small, footWidth, text }: HandMarksProps) {
  return (
    <>
      {revealed ? <RevealedMark compact={small || footWidth < REVEALED_WORD_PX} text={text} /> : null}
      {note === undefined ? null : (
        <NoteMark note={note} compact={small || footWidth < NOTE_TEXT_PX} text={text} />
      )}
    </>
  )
}

/** A note a seat put on the card: a sticky note with its first words, named by the whole note; the icon alone on a compact face. */
export function NoteMark({ note, compact, text }: MarkProps & { note: string }) {
  return (
    <span
      role="img"
      aria-label={noteName(note)}
      title={noteName(note)}
      data-slot="card-hand-note"
      className={cn(MARK, 'border border-foreground/40 bg-secondary text-secondary-foreground', text)}
    >
      <StickyNote aria-hidden="true" className="size-3 shrink-0" />
      {compact ? null : <span className="truncate">{noteLead(note)}</span>}
    </span>
  )
}
