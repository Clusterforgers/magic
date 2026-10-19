import { Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import type { DeckInfo } from '@/net/api'
import { DeckPreview } from './DeckPreview'
import { nameFieldId } from './form'
import type { SeatDraft, SeatProblems } from './form'

interface SeatFieldsProps {
  /** The seat's place in the list, from 1. */
  position: number
  seat: SeatDraft
  decks: readonly DeckInfo[]
  /** What to show as wrong, or null to show nothing yet. */
  problems: SeatProblems | null
  canRemove: boolean
  onEdit(id: number, change: Partial<Pick<SeatDraft, 'name' | 'deck'>>): void
  onRemove(id: number): void
}

/** One seat of the form: a name, a deck with its preview, and a way to take the seat out. */
export function SeatFields({
  position,
  seat,
  decks,
  problems,
  canRemove,
  onEdit,
  onRemove,
}: SeatFieldsProps) {
  const nameId = nameFieldId(seat.id)
  const deckId = `seat-${seat.id}-deck`
  const deck = decks.find((candidate) => candidate.id === seat.deck)
  const nameProblem = problems?.name ?? null
  const deckProblem = problems?.deck ?? null
  return (
    <li className="surface p-4">
      <div className="mb-3 flex items-center justify-between gap-2">
        <h3 className="text-sm font-medium">Seat {position}</h3>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          disabled={!canRemove}
          aria-label={`Remove seat ${position}`}
          onClick={() => onRemove(seat.id)}
        >
          <Trash2 aria-hidden="true" />
          Remove
        </Button>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="grid content-start gap-2">
          <Label htmlFor={nameId}>Name</Label>
          <Input
            id={nameId}
            aria-label={`Name for seat ${position}`}
            value={seat.name}
            autoComplete="off"
            aria-invalid={nameProblem !== null}
            aria-describedby={nameProblem === null ? undefined : `${nameId}-problem`}
            onChange={(event) => onEdit(seat.id, { name: event.target.value })}
          />
          {nameProblem === null ? null : (
            <p id={`${nameId}-problem`} className="text-sm text-destructive">
              {nameProblem}
            </p>
          )}
        </div>
        <div className="grid content-start gap-2">
          <Label htmlFor={deckId}>Deck</Label>
          <NativeSelect
            id={deckId}
            aria-label={`Deck for seat ${position}`}
            className="w-full"
            value={seat.deck}
            aria-invalid={deckProblem !== null}
            aria-describedby={deckProblem === null ? undefined : `${deckId}-problem`}
            onChange={(event) => onEdit(seat.id, { deck: event.target.value })}
          >
            {deck === undefined ? <NativeSelectOption value="">Choose a deck</NativeSelectOption> : null}
            {decks.map((option) => (
              <NativeSelectOption key={option.id} value={option.id}>
                {option.name} ({option.color}, {option.size} cards)
              </NativeSelectOption>
            ))}
          </NativeSelect>
          {deckProblem === null ? null : (
            <p id={`${deckId}-problem`} className="text-sm text-destructive">
              {deckProblem}
            </p>
          )}
        </div>
      </div>
      {deck === undefined ? null : (
        <div className="mt-3">
          <DeckPreview deck={deck} />
        </div>
      )}
    </li>
  )
}
