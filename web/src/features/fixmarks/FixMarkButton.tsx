import { FlagIcon, PencilLineIcon } from 'lucide-react'
import { useId, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import type { FixMarksApi } from '@/net/fixMarks'
import { describeMarkError } from './errors'
import { useFixMarks } from './useFixMarks'

/** The longest note of a mark (`MAX_FIX_NOTE_CHARS` of `mtg_protocol::rest`). */
export const MAX_NOTE_CHARS = 280

interface FixMarkButtonProps {
  /** The card to mark. */
  oracleId: string
  /** Its name, kept in the list while the server answers. */
  name: string
  /** The fix-mark endpoints. */
  api: FixMarksApi
  /** `prominent` for a card the game does not run whole, `quiet` for any other card. */
  emphasis?: 'prominent' | 'quiet'
  /** An icon alone for a row of a list: no words, no note; a refusal floats under the icon. */
  compact?: boolean
  className?: string
}

/** The popover that takes the note of a mark: what is wrong with the card, up to 280 characters, optional. */
function NoteEditor({
  note,
  disabled,
  onSave,
}: {
  note: string | null
  disabled: boolean
  onSave: (note: string | null) => void
}) {
  const [open, setOpen] = useState(false)
  const [draft, setDraft] = useState('')
  const field = useId()
  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        if (next) setDraft(note ?? '')
        setOpen(next)
      }}
    >
      <PopoverTrigger render={<Button type="button" variant="ghost" size="sm" />}>
        <PencilLineIcon aria-hidden="true" />
        {note === null ? 'Add note' : 'Edit note'}
      </PopoverTrigger>
      <PopoverContent align="start">
        <PopoverTitle>What is wrong with this card?</PopoverTitle>
        <form
          className="grid gap-3"
          onSubmit={(event) => {
            event.preventDefault()
            onSave(draft.trim() === '' ? null : draft.trim())
            setOpen(false)
          }}
        >
          <label htmlFor={field} className="text-sm text-muted-foreground">
            Optional. It helps whoever fixes the card.
          </label>
          <textarea
            id={field}
            value={draft}
            rows={3}
            maxLength={MAX_NOTE_CHARS}
            className="min-h-20 w-full resize-y rounded-lg border border-input bg-background px-3 py-2 text-base text-foreground placeholder:text-muted-foreground"
            onChange={(event) => setDraft(event.target.value)}
          />
          <p className="text-xs text-muted-foreground tabular-nums" aria-live="off">
            {draft.length} of {MAX_NOTE_CHARS}
          </p>
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" size="sm" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" size="sm" aria-disabled={disabled}>
              Save note
            </Button>
          </div>
        </form>
      </PopoverContent>
    </Popover>
  )
}

/**
 * A toggle that marks a card as important to fix (docs/web.md, Fix marks): pressed when the account has marked the card.
 * A press changes the list at once and the server confirms it; a refusal puts it back and says why under the button. A
 * marked card also offers an optional note of up to 280 characters in a popover. Nothing is shown to a visitor without
 * a session or on a server without a card database.
 */
export function FixMarkButton({
  oracleId,
  name,
  api,
  emphasis = 'prominent',
  compact = false,
  className,
}: FixMarkButtonProps) {
  const marks = useFixMarks(api)
  const mark = marks.markOf(oracleId)
  const marked = mark !== undefined
  if (marks.unavailable) return null
  const busy = marks.loading || marks.pending
  if (compact) {
    return (
      <span data-slot="fix-mark" data-compact className={cn('relative inline-flex', className)}>
        <Button
          type="button"
          size="icon-sm"
          variant={marked ? 'secondary' : 'ghost'}
          aria-label={`Mark ${name} important to fix`}
          aria-pressed={marked}
          aria-disabled={busy}
          title={marked ? 'Marked important to fix' : 'Mark important to fix'}
          className={marked ? undefined : 'text-muted-foreground'}
          onClick={() => {
            if (busy) return
            marks.clearError()
            marks.press({ oracleId, name, marked: !marked, note: null })
          }}
        >
          <FlagIcon aria-hidden="true" className={marked ? 'fill-current' : undefined} />
        </Button>
        {marks.error === null ? null : (
          <p
            role="alert"
            className="absolute top-full right-0 z-20 mt-1 w-56 rounded-md border border-destructive bg-popover p-2 text-xs text-destructive shadow-e2"
          >
            {describeMarkError(marks.error)}
          </p>
        )}
      </span>
    )
  }
  return (
    <div data-slot="fix-mark" className={cn('grid justify-items-start gap-1', className)}>
      <div className="flex flex-wrap items-center gap-1">
        <Button
          type="button"
          size="sm"
          variant={marked ? 'secondary' : emphasis === 'prominent' ? 'outline' : 'ghost'}
          aria-pressed={marked}
          aria-disabled={busy}
          className={emphasis === 'quiet' && !marked ? 'text-muted-foreground' : undefined}
          onClick={() => {
            if (busy) return
            marks.clearError()
            marks.press({ oracleId, name, marked: !marked, note: null })
          }}
        >
          <FlagIcon aria-hidden="true" className={marked ? 'fill-current' : undefined} />
          Mark important to fix
        </Button>
        {marked ? (
          <NoteEditor
            note={mark.note}
            disabled={marks.pending}
            onSave={(note) => marks.press({ oracleId, name, marked: true, note })}
          />
        ) : null}
      </div>
      {marked && mark.note !== null ? (
        <p data-slot="fix-mark-note" className="text-sm text-muted-foreground">
          Your note: {mark.note}
        </p>
      ) : null}
      {marks.error === null ? null : (
        <p role="alert" className="text-sm text-destructive">
          {describeMarkError(marks.error)}
        </p>
      )}
    </div>
  )
}
