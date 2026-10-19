import { Trash2Icon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { timeSince } from '@/features/games/format'
import type { DecksApi } from '@/net/decks'
import type { DeckListItem } from '@/protocol/generated'
import { DeckStateBadge } from './DeckStateBadge'
import { ExportMenu } from './ExportMenu'
import { cardsText, checkNote, formatLabel } from './format'

interface DeckRowProps {
  deck: DeckListItem
  api: DecksApi
  /** The time the ages are counted to, in milliseconds since the epoch. */
  now: number
  onDelete: (deck: DeckListItem) => void
}

/**
 * One deck of the list: its name, check state (an icon and a word), format, card count and age, a line that says what
 * the state means for this deck, and the actions Export and Delete.
 */
export function DeckRow({ deck, api, now, onDelete }: DeckRowProps) {
  const note = checkNote(deck)
  return (
    <li
      data-deck={deck.id}
      data-state={deck.state}
      className="surface grid gap-3 p-4 sm:grid-cols-[1fr_auto] sm:items-center"
    >
      <div className="grid min-w-0 gap-1">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <p className="min-w-0 text-base font-semibold break-words">{deck.name}</p>
          <DeckStateBadge state={deck.state} />
        </div>
        <p className="text-sm text-muted-foreground">
          {formatLabel(deck.format)} · {cardsText(deck.summary.counts.total)} · Saved{' '}
          <time dateTime={deck.updated_at} title={deck.updated_at}>
            {timeSince(deck.updated_at, now)}
          </time>
        </p>
        {note === null ? null : <p className="text-sm">{note}</p>}
      </div>
      <div className="flex items-center gap-2">
        <ExportMenu deck={deck} api={api} />
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-haspopup="dialog"
          aria-label={`Delete ${deck.name}`}
          onClick={() => onDelete(deck)}
        >
          <Trash2Icon aria-hidden="true" />
          Delete
        </Button>
      </div>
    </li>
  )
}
