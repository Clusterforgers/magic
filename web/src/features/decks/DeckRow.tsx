import { Link } from '@tanstack/react-router'
import { PencilIcon, Trash2Icon } from 'lucide-react'
import { Button, buttonVariants } from '@/components/ui/button'
import { CardPicture } from '@/features/cards/CardPicture'
import { timeSince } from '@/lib/timeSince'
import type { DecksApi } from '@/net/decks'
import type { DeckListItem } from '@/protocol/generated'
import { DeckMarker } from './DeckMarker'
import { ExportMenu } from './ExportMenu'
import { cardsText, checkNote, formatLabel, hasUnvalidated } from './format'

interface DeckRowProps {
  deck: DeckListItem
  api: DecksApi
  /** The time the ages are counted to, in milliseconds since the epoch. */
  now: number
  onDelete: (deck: DeckListItem) => void
}

/**
 * One deck of the list: its name (a link to the builder), the marker beside it (an icon and a word, or "Changed since
 * the last check"), format, card count and age, a line that says what the state means for this deck, and the actions
 * Edit, Export and Delete.
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
          <p className="min-w-0 text-base font-semibold [overflow-wrap:anywhere]">
            <Link
              to="/decks/$deckId"
              params={{ deckId: deck.id }}
              className="underline-offset-4 hover:underline"
            >
              {deck.name}
            </Link>
          </p>
          <DeckMarker state={deck.state} changed={deck.changed} />
        </div>
        {deck.commanders.length === 0 ? null : (
          <ul aria-label="Commanders" data-slot="commanders" className="flex flex-wrap gap-x-4 gap-y-2 py-1">
            {deck.commanders.map((card) => (
              <li
                key={card.oracle_id}
                data-commander={card.oracle_id}
                className="flex min-w-0 items-center gap-2"
              >
                <CardPicture
                  oracleId={card.oracle_id}
                  name={card.name}
                  size="small"
                  alt=""
                  className="w-10"
                />
                <span data-slot="card-name" className="min-w-0 text-sm font-medium [overflow-wrap:anywhere]">
                  {card.name}
                </span>
              </li>
            ))}
          </ul>
        )}
        <p className="text-sm text-muted-foreground">
          {formatLabel(deck.format)} · {cardsText(deck.summary.counts.total)} · Saved{' '}
          <time dateTime={deck.updated_at} title={deck.updated_at}>
            {timeSince(deck.updated_at, now)}
          </time>
        </p>
        {note === null ? null : <p className="text-sm">{note}</p>}
        {hasUnvalidated(deck.summary.counts) ? (
          <Link
            to="/decks/$deckId/unvalidated"
            params={{ deckId: deck.id }}
            aria-label={`Not validated cards of ${deck.name}`}
            className="w-fit text-sm text-muted-foreground underline-offset-4 hover:underline"
          >
            Not validated cards
          </Link>
        ) : null}
      </div>
      <div className="flex items-center gap-2">
        <Link
          to="/decks/$deckId"
          params={{ deckId: deck.id }}
          aria-label={`Edit ${deck.name}`}
          className={buttonVariants({ variant: 'outline', size: 'sm' })}
        >
          <PencilIcon aria-hidden="true" />
          Edit
        </Link>
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
