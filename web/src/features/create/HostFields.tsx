import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import type { DeckInfo } from '@/net/api'
import { DeckPreview } from './DeckPreview'
import { SeatCountField } from './SeatCountField'
import { nameFieldId } from './form'
import type { Draft, SeatDraft, SeatProblems } from './form'

interface HostFieldsProps {
  draft: Draft
  decks: readonly DeckInfo[]
  /** What to show as wrong with the creator's seat, or null to show nothing yet. */
  problems: SeatProblems | null
  onEdit(id: number, change: Partial<Pick<SeatDraft, 'name' | 'deck'>>): void
  /** The number of seats changed. */
  onCount(count: number): void
}

/** How many seats there are and the creator's own name and deck; the other seats are open for friends to take. */
export function HostFields({ draft, decks, problems, onEdit, onCount }: HostFieldsProps) {
  const seat = draft.seats[0]
  if (seat === undefined) return null
  const nameId = nameFieldId(seat.id)
  const deckId = `seat-${seat.id}-deck`
  const deck = decks.find((candidate) => candidate.id === seat.deck)
  const nameProblem = problems?.name ?? null
  const deckProblem = problems?.deck ?? null
  return (
    <section aria-labelledby="players-title" className="grid content-start gap-3">
      <h2 id="players-title" className="text-lg font-semibold">
        Players
      </h2>
      <div className="grid items-start gap-4 lg:grid-cols-2">
        <SeatCountField count={draft.seats.length} onCount={onCount} />
        <div className="grid gap-2">
          <Label htmlFor={nameId}>Your name</Label>
          <Input
            id={nameId}
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
      </div>
      {draft.format === 'normal' ? (
        <div className="grid gap-2">
          <Label htmlFor={deckId}>Your deck</Label>
          <NativeSelect
            id={deckId}
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
          {deck === undefined ? null : <DeckPreview deck={deck} />}
        </div>
      ) : (
        <p data-slot="deck-later" className="text-sm text-muted-foreground">
          You choose your deck in the lobby.
        </p>
      )}
    </section>
  )
}
