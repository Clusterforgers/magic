import { ShieldCheckIcon } from 'lucide-react'
import { CardPicture } from '@/features/cards/CardPicture'
import { DraftNote } from '@/features/decks/DraftNote'
import { ClauseList } from '@/features/decks/unvalidated/ClauseList'
import { clauseLines } from '@/features/decks/unvalidated/model'
import type { FaceTexts } from '@/features/decks/unvalidated/model'
import { useCardTexts } from '@/features/decks/unvalidated/useCardTexts'
import { SupportMark, supportEdge } from '@/features/decks/SupportMark'
import { FixMarkButton } from '@/features/fixmarks/FixMarkButton'
import { BY_HAND, BY_HAND_TITLE, runsWhole } from '@/lib/byHand'
import { cn } from '@/lib/utils'
import type { CardsApi } from '@/net/cards'
import type { FixMarksApi } from '@/net/fixMarks'
import type { CardHit } from '@/protocol/generated'
import type { HandCard } from './deckSummary'

interface HandListProps {
  /** The cards that are not Verified, worst state first (`handCards`). */
  cards: readonly HandCard[]
  /** Opens the preview of a card; `opener` is the control focus returns to. */
  onPreview: (card: CardHit, opener: HTMLElement) => void
  fixMarks?: FixMarksApi
  /** The card endpoint the words each reason covers are quoted from. */
  cardsApi: Pick<CardsApi, 'get'>
}

function Entry({
  entry,
  onPreview,
  fixMarks,
  faces,
  pending,
}: {
  entry: HandCard
  onPreview: HandListProps['onPreview']
  fixMarks?: FixMarksApi
  faces: FaceTexts
  pending: boolean
}) {
  const { card, count, support, draft, reasons, clauses } = entry
  return (
    <li
      data-hand={card.oracle_id}
      data-support={support}
      className={cn('flex gap-3 rounded-lg border border-l-4 p-2', supportEdge(support))}
    >
      <CardPicture
        oracleId={card.oracle_id}
        name={card.name}
        size="small"
        alt=""
        quiet
        className="w-12 sm:w-16"
      />
      <div className="grid min-w-0 flex-1 content-start gap-1">
        <div className="flex items-start gap-2">
          <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">
            <button
              type="button"
              aria-haspopup="dialog"
              title={card.name}
              data-slot="card-name"
              onClick={(event) => onPreview(card, event.currentTarget)}
              className="min-h-6 max-w-full min-w-0 truncate rounded-sm text-left text-sm font-medium underline-offset-4 hover:underline"
            >
              {card.name}
              {count > 1 ? <span className="font-normal text-muted-foreground"> ×{count}</span> : null}
            </button>
            <SupportMark support={support} />
          </div>
          {fixMarks === undefined ? null : (
            <FixMarkButton compact oracleId={card.oracle_id} name={card.name} api={fixMarks} />
          )}
        </div>
        {reasons.length === 0 || runsWhole(support) ? (
          <p className="text-xs text-muted-foreground">The reasons appear once the deck is checked.</p>
        ) : (
          <div aria-label={`Why ${card.name} is ${BY_HAND}`} role="group">
            <ClauseList
              lines={clauseLines(
                { oracleId: card.oracle_id, name: card.name, support, reasons: clauses, decks: [] },
                faces.get(card.oracle_id),
              )}
              support={support}
              pending={pending}
              stacked
            />
          </div>
        )}
        <DraftNote draft={draft} />
      </div>
    </li>
  )
}

/**
 * The cards the game leaves to the table: each with its picture, state, the clauses and reasons the check gives and the
 * flag that marks it important to fix, so the player sees what is played by hand and can build the deck around
 * it. A card the game cannot play at all is listed apart, since the deck cannot be taken with it.
 */
export function HandList({ cards, cardsApi, onPreview, fixMarks }: HandListProps) {
  const { faces, pending } = useCardTexts(
    cardsApi,
    cards.filter((entry) => entry.reasons.length > 0).map((entry) => entry.card.oracle_id),
  )
  const hand = cards.filter((entry) => entry.support !== 'unplayable')
  const blocked = cards.filter((entry) => entry.support === 'unplayable')
  const copies = (list: readonly HandCard[]): number => list.reduce((sum, entry) => sum + entry.count, 0)
  return (
    <section aria-label={BY_HAND_TITLE} data-slot="hand-list" className="@container grid gap-2">
      {cards.length === 0 ? (
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <ShieldCheckIcon aria-hidden="true" className="size-4 text-support-verified" />
          Every card runs whole.
        </p>
      ) : (
        <>
          {hand.length === 0 ? null : (
            <ul aria-label={`Cards ${BY_HAND}`} className="grid gap-2 @3xl:grid-cols-2">
              {hand.map((entry) => (
                <Entry
                  key={entry.card.oracle_id}
                  entry={entry}
                  onPreview={onPreview}
                  fixMarks={fixMarks}
                  faces={faces}
                  pending={pending}
                />
              ))}
            </ul>
          )}
          {blocked.length === 0 ? null : (
            <>
              <h4 className="flex items-baseline justify-between gap-2 pt-1 text-xs font-medium">
                Cannot be played
                <span className="font-normal text-muted-foreground tabular-nums">{copies(blocked)}</span>
              </h4>
              <ul aria-label="Cards the game cannot play" className="grid gap-2">
                {blocked.map((entry) => (
                  <Entry
                    key={entry.card.oracle_id}
                    entry={entry}
                    onPreview={onPreview}
                    fixMarks={fixMarks}
                    faces={faces}
                    pending={pending}
                  />
                ))}
              </ul>
            </>
          )}
        </>
      )}
    </section>
  )
}
