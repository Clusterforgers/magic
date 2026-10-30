/** The marks at a host's foot for what its attachments give: one per attachment, "+N" past the room, a count and an icon when compact. */
import { Ban, ChevronsUp, Link2 } from 'lucide-react'
import { cn } from '@/lib/utils'
import { capMarks, compactTone, marksFrom } from './attachMarks'
import type { Attach, MarkTone } from './attachMarks'

const MARK =
  'flex h-fit max-w-full items-center gap-0.5 rounded-sm px-1 leading-4 font-semibold whitespace-nowrap shadow-sm'

// Tokens only: a raise on `success`, a lowering or a restriction on `destructive`, anything else on `surface`.
const TONE: Record<MarkTone, string> = {
  raise: 'bg-success text-background',
  restrict: 'bg-destructive text-background',
  plain: 'bg-surface text-foreground ring-1 ring-border-strong',
}

// A count or a "+N" is no raise and no restriction: the colour stays with the marks it stands for.
const COUNT = 'bg-background text-foreground ring-1 ring-foreground'

// Another seat's attachment has the dashed edge of its tucked card.
const FOREIGN = 'outline-1 outline-offset-1 outline-dashed outline-foreground'

const ICON: Record<MarkTone, typeof Link2> = { raise: ChevronsUp, restrict: Ban, plain: Link2 }

// A mark's padding and the width of a character of 11 px semibold type.
const PAD_PX = 8
const CHAR_PX = 6.4

/** The characters a mark may write in `span` px: fewer items are written when they do not fit. */
function charsFor(span: number): number {
  return Math.max(6, Math.floor((span - PAD_PX) / CHAR_PX))
}

interface GrantMarksProps {
  attaches: readonly Attach[]
  /** A compact face shows the count and one icon instead of the marks. */
  compact: boolean
  /** The lines the stack has for the marks; fewer than 2 shows the count too. */
  room: number
  /** The width in px a mark may use, which decides how much of it is written. */
  span: number
  /** The stack grows upward (the first child is at the bottom), so the first attachment is the last child. */
  up?: boolean
  /** The text size class of the face. */
  text: string
}

/** The grant marks of a host, or nothing while it carries no attachment. */
export function GrantMarks({ attaches, compact, room, span, up = false, text }: GrantMarksProps) {
  if (attaches.length === 0) return null
  const marks = marksFrom(attaches, { chars: charsFor(span) })
  if (compact || room < 2) {
    const tone = compactTone(marks)
    const Icon = ICON[tone]
    return (
      <span data-slot="card-grants" data-tone={tone} className={cn(MARK, COUNT, text)}>
        <Icon aria-hidden="true" className="size-3 shrink-0" />
        {attaches.length}
      </span>
    )
  }
  const { shown, more } = capMarks(marks, room)
  const parts = shown.map((mark) => (
    <span
      key={mark.key}
      data-slot="card-grant"
      data-tone={mark.tone}
      data-foreign={mark.foreign.length > 0 || undefined}
      className={cn(MARK, TONE[mark.tone], mark.foreign.length > 0 && FOREIGN, text)}
    >
      <span className="truncate">{mark.text}</span>
      {mark.ids.length > 1 ? <span className="shrink-0">×{mark.ids.length}</span> : null}
    </span>
  ))
  if (more > 0) {
    parts.push(
      <span key="more" data-slot="card-grants-more" className={cn(MARK, COUNT, text)}>
        +{more}
      </span>,
    )
  }
  // The marks read top to bottom as the tucked cards do.
  return <>{up ? parts.reverse() : parts}</>
}
