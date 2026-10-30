/** The marks at a host's foot for what its attachments give: one per attachment, the rest summed past the room, a count and an icon when compact. */
import { Fragment } from 'react'
import { Ban, ChevronsUp, Link2 } from 'lucide-react'
import { cn } from '@/lib/utils'
import { compactTone, foldMarks, marksFrom } from './attachMarks'
import type { Attach, Mark, MarkTone } from './attachMarks'

const MARK =
  'flex h-fit max-w-full items-center gap-0.5 rounded-sm px-1 leading-4 font-semibold whitespace-nowrap shadow-sm'

// A mark is 16 px high a line: the line height comes after the text size, which tailwind-merge would otherwise let drop it.
const LINE = 'leading-4'

// Tokens only: a raise on `success`, a lowering or a restriction on `destructive`, anything else on `surface`.
const TONE: Record<MarkTone, string> = {
  raise: 'bg-success text-background',
  restrict: 'bg-destructive text-background',
  plain: 'bg-surface text-foreground ring-1 ring-border-strong',
}

// A count is no raise and no restriction, and the sum of the attachments past the room is the rest of them: neither
// takes a tone's colour, so the marks that are shown stay the ones with a line of their own.
const COUNT = 'bg-background text-foreground ring-1 ring-foreground'

// Another seat's attachment has the dashed edge of its tucked card.
const FOREIGN = 'outline-1 outline-offset-1 outline-dashed outline-foreground'

const ICON: Record<MarkTone, typeof Link2> = { raise: ChevronsUp, restrict: Ban, plain: Link2 }

/** The text size in px of a face's text class (`cardSize.ts`), which decides how wide its words are. */
function fontPx(text: string): number {
  if (text.includes('text-xs')) return 12
  return text.includes('0.7rem') ? 11.2 : 11
}

/** The lines of a mark, each whole words and none cut by the box; a space between them keeps the text one string. */
function Lines({ lines }: { lines: readonly string[] }) {
  return (
    <span className="min-w-0">
      {lines.map((line, at) => (
        <Fragment key={`${at}:${line}`}>
          {at === 0 ? null : ' '}
          <span className="block truncate">{line}</span>
        </Fragment>
      ))}
    </span>
  )
}

interface GrantMarksProps {
  attaches: readonly Attach[]
  /** A compact face shows the count and one icon instead of the marks. */
  compact: boolean
  /** The lines the stack has for the marks; fewer than 2 shows the count too. */
  room: number
  /** The width in px a mark may use, which decides which words it writes and in how many lines. */
  span: number
  /** The stack grows upward (the first child is at the bottom), so the first attachment is the last child. */
  up?: boolean
  /** The text size class of the face. */
  text: string
}

/** The grant marks of a host, or nothing while it carries no attachment. */
export function GrantMarks({ attaches, compact, room, span, up = false, text }: GrantMarksProps) {
  if (attaches.length === 0) return null
  const options = { span, fontPx: fontPx(text) }
  if (compact || room < 2) {
    const tone = compactTone(marksFrom(attaches, options))
    const Icon = ICON[tone]
    return (
      <span data-slot="card-grants" data-tone={tone} className={cn(MARK, COUNT, 'gap-px px-0.5', text, LINE)}>
        <Icon aria-hidden="true" className="size-3 shrink-0" />
        {attaches.length}
      </span>
    )
  }
  const { shown, rest } = foldMarks(attaches, room, options)
  const parts = shown.map((mark: Mark) => (
    <span
      key={mark.key}
      data-slot="card-grant"
      data-tone={mark.tone}
      data-foreign={mark.foreign.length > 0 || undefined}
      className={cn(MARK, TONE[mark.tone], mark.foreign.length > 0 && FOREIGN, text, LINE)}
    >
      <Lines lines={mark.lines} />
      {mark.ids.length > 1 ? <span className="shrink-0">×{mark.ids.length}</span> : null}
    </span>
  ))
  if (rest !== null) {
    parts.push(
      <span
        key="more"
        data-slot="card-grants-more"
        data-tone={rest.tone}
        title={rest.words}
        className={cn(MARK, COUNT, text, LINE)}
      >
        <Lines lines={rest.lines} />
      </span>,
    )
  }
  // The marks read top to bottom as the tucked cards do.
  return <>{up ? parts.reverse() : parts}</>
}
