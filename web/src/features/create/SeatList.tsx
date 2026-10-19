import { Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { MAX_SEATS } from '@/lib/limits'
import type { DeckInfo } from '@/net/api'
import { SeatFields } from './SeatFields'
import type { Draft, SeatDraft, SeatProblems } from './form'

interface SeatListProps {
  draft: Draft
  decks: readonly DeckInfo[]
  /** What is wrong with each seat by id, or null to show nothing yet. */
  problems: ReadonlyMap<number, SeatProblems> | null
  onEdit(id: number, change: Partial<Pick<SeatDraft, 'name' | 'deck'>>): void
  onAdd(): void
  onRemove(id: number): void
}

/** Every seat with a name and a deck of its own, for a game that gives each seat a link. */
export function SeatList({ draft, decks, problems, onEdit, onAdd, onRemove }: SeatListProps) {
  return (
    <section aria-labelledby="seats-title" className="grid content-start gap-3">
      <div className="flex items-center justify-between gap-2">
        <h2 id="seats-title" className="text-lg font-semibold">
          Seats
        </h2>
        <p className="text-sm text-muted-foreground" aria-live="polite">
          {draft.seats.length} of {MAX_SEATS} seats
        </p>
      </div>
      <ol className="grid gap-3">
        {draft.seats.map((seat, index) => (
          <SeatFields
            key={seat.id}
            position={index + 1}
            seat={seat}
            decks={decks}
            problems={problems?.get(seat.id) ?? null}
            canRemove={draft.seats.length > 2}
            onEdit={onEdit}
            onRemove={onRemove}
          />
        ))}
      </ol>
      <div>
        <Button type="button" variant="outline" disabled={draft.seats.length >= MAX_SEATS} onClick={onAdd}>
          <Plus aria-hidden="true" />
          Add seat
        </Button>
      </div>
    </section>
  )
}
