import { ShieldCheckIcon } from 'lucide-react'
import { CardPicture } from '@/features/cards/CardPicture'
import { DraftNote } from '@/features/decks/DraftNote'
import { SupportMark, supportEdge } from '@/features/decks/SupportMark'
import { FixMarkButton } from '@/features/fixmarks/FixMarkButton'
import { BY_HAND, BY_HAND_TITLE } from '@/lib/byHand'
import { cn } from '@/lib/utils'
import type { FixMarksApi } from '@/net/fixMarks'
import type { CardHit } from '@/protocol/generated'
import type { HandCard } from './deckSummary'

interface HandListProps {
  /** The cards that are not Verified, worst state first (`handCards`). */
  cards: readonly HandCard[]
  /** Opens the preview of a card; `opener` is the control focus returns to. */
  onPreview: (card: CardHit, opener: HTMLElement) => void
  fixMarks?: FixMarksApi
}

function Entry({
  entry,
  onPreview,
  fixMarks,
}: {
  entry: HandCard
  onPreview: HandListProps['onPreview']
  fixMarks?: FixMarksApi
}) {
  const { card, count, support, draft, reasons } = entry
  return (
    <li
      data-hand={card.oracle_id}
      data-support={support}
      className={cn('flex gap-3 rounded-lg border-l-4 py-1 pr-1 pl-2', supportEdge(support))}
    >
      <CardPicture oracleId={card.oracle_id} name={card.name} size="small" alt="" quiet className="w-16" />
      <div className="grid min-w-0 flex-1 content-start gap-1">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
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
          {fixMarks === undefined ? null : (
            <FixMarkButton compact oracleId={card.oracle_id} name={card.name} api={fixMarks} />
          )}
        </div>
        {reasons.length === 0 ? (
          <p className="text-xs text-muted-foreground">The reasons appear once the deck is checked.</p>
        ) : (
          <ul aria-label={`Why ${card.name} is ${BY_HAND}`} className="grid gap-1 text-xs">
            {[...new Set(reasons)].map((reason) => (
              <li key={reason} className="text-muted-foreground [overflow-wrap:anywhere]">
                {reason}
              </li>
            ))}
          </ul>
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
export function HandList({ cards, onPreview, fixMarks }: HandListProps) {
  const hand = cards.filter((entry) => entry.support !== 'unplayable')
  const blocked = cards.filter((entry) => entry.support === 'unplayable')
  const copies = (list: readonly HandCard[]): number => list.reduce((sum, entry) => sum + entry.count, 0)
  return (
    <section aria-label={BY_HAND_TITLE} data-slot="hand-list" className="grid gap-2">
      {cards.length === 0 ? (
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <ShieldCheckIcon aria-hidden="true" className="size-4 text-support-verified" />
          Every card runs whole.
        </p>
      ) : (
        <>
          {hand.length === 0 ? null : (
            <ul aria-label={`Cards ${BY_HAND}`} className="grid gap-2">
              {hand.map((entry) => (
                <Entry key={entry.card.oracle_id} entry={entry} onPreview={onPreview} fixMarks={fixMarks} />
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
                  <Entry key={entry.card.oracle_id} entry={entry} onPreview={onPreview} fixMarks={fixMarks} />
                ))}
              </ul>
            </>
          )}
        </>
      )}
    </section>
  )
}
