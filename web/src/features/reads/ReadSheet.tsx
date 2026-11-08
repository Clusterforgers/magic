import { CheckIcon, HandIcon, TriangleAlertIcon } from 'lucide-react'
import type { ReadRow, ReadSheet as Sheet } from '@/protocol/generated'
import { cn } from '@/lib/utils'
import { TONE_WORDS, byHandReason, engineWords, toneOf } from './model'
import type { RowTone } from './model'

const TONE_EDGE: Readonly<Record<RowTone, string>> = {
  written: 'border-border',
  unwritten: 'border-border',
  differs: 'border-l-4 border-support-partial bg-support-partial/10',
  by_hand: 'border-l-4 border-support-manual',
  no_clause: 'border-l-4 border-support-partial bg-support-partial/10',
}

// A step under an "if" or a mode sits one indent deeper per block.
const DEPTH = ['ps-0', 'ps-4', 'ps-8', 'ps-12'] as const

function ToneMark({ tone }: { tone: RowTone }) {
  const Icon = tone === 'written' ? CheckIcon : tone === 'by_hand' ? HandIcon : TriangleAlertIcon
  const colour =
    tone === 'written'
      ? 'text-support-verified'
      : tone === 'by_hand'
        ? 'text-support-manual'
        : tone === 'unwritten'
          ? 'text-muted-foreground'
          : 'text-support-partial'
  return (
    <p className="flex items-center gap-1 text-xs font-medium text-foreground">
      <Icon aria-hidden="true" className={cn('size-3.5 shrink-0', colour)} />
      {TONE_WORDS[tone]}
    </p>
  )
}

/** What the game makes of one clause: its wording, then the steps it takes, indented by depth. */
function EngineSide({ row }: { row: ReadRow }) {
  const tone = toneOf(row)
  if (tone === 'by_hand') {
    const reason = byHandReason(row)
    return (
      <p className="text-sm text-muted-foreground">
        The game asks nothing here; the players apply it.{reason === null ? '' : ` ${reason}.`}
      </p>
    )
  }
  if (row.continues) {
    return <p className="text-sm text-muted-foreground">The same ability as the clause above.</p>
  }
  const words = engineWords(row)
  return (
    <div className="grid gap-1">
      {words === null ? null : (
        <blockquote
          data-slot="read-engine"
          className="text-sm break-words text-foreground"
        >{`“${words}”`}</blockquote>
      )}
      <ol aria-label="What the game does" className="grid gap-0.5 text-sm text-muted-foreground">
        {row.steps.map((step, index) => (
          <li key={index} className={cn('break-words', DEPTH[Math.min(step.depth, DEPTH.length - 1)])}>
            {step.text}
          </li>
        ))}
      </ol>
      <p className="text-xs text-muted-foreground">{row.label}</p>
    </div>
  )
}

function SheetRow({ row, faced }: { row: ReadRow; faced: boolean }) {
  const tone = toneOf(row)
  return (
    <li
      data-tone={tone}
      className={cn('grid gap-2 rounded-md border p-3 sm:grid-cols-2 sm:gap-4', TONE_EDGE[tone])}
    >
      <div className="min-w-0">
        <p className="text-xs text-muted-foreground">{faced ? `Printed, face ${row.face + 1}` : 'Printed'}</p>
        {row.printed === '' ? (
          <p className="text-sm text-muted-foreground">Nothing printed stands for this ability.</p>
        ) : (
          <blockquote className="text-sm break-words whitespace-pre-line text-foreground">{`“${row.printed}”`}</blockquote>
        )}
      </div>
      <div className="grid min-w-0 content-start gap-1">
        <ToneMark tone={tone} />
        <EngineSide row={row} />
      </div>
    </li>
  )
}

/**
 * The read sheet of a card: every printed clause beside the engine's own English and the steps it takes, in text order
 * (stacked under `sm`). A clause the draft words differently is highlighted, so a reader compares it before judging.
 */
export function ReadSheet({ sheet }: { sheet: Sheet }) {
  const faced = sheet.faces.length > 1
  return (
    <div className="grid gap-3">
      <ul aria-label="Clauses beside the game's reading" className="grid gap-2">
        {sheet.rows.map((row) => (
          <SheetRow key={`${row.face}:${row.span.start}`} row={row} faced={faced} />
        ))}
      </ul>
      {sheet.extra.length === 0 ? null : (
        <section aria-label="Abilities not on the card" className="grid gap-2">
          <p className="text-sm font-medium">The rules also hold abilities no clause stands for</p>
          <ul className="grid gap-2">
            {sheet.extra.map((row, index) => (
              <SheetRow key={index} row={{ ...row, state: 'no_clause' }} faced={faced} />
            ))}
          </ul>
        </section>
      )}
    </div>
  )
}
