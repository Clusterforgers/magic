import { Link } from '@tanstack/react-router'
import { CardPicture } from '@/features/cards/CardPicture'
import { FixMarkButton } from '@/features/fixmarks/FixMarkButton'
import { cn } from '@/lib/utils'
import type { FixMarksApi } from '@/net/fixMarks'
import { DraftNote } from '../DraftNote'
import { SupportMark, supportEdge } from '../SupportMark'
import { ClauseList } from './ClauseList'
import { clauseLines } from './model'
import type { FlaggedCard } from './model'

interface UnvalidatedCardProps {
  card: FlaggedCard
  /** The oracle text of the card, one string per face, once read. */
  faces: readonly string[] | undefined
  /** The card's text is still being read. */
  pending: boolean
  /** Name the decks that hold the card under its name. */
  showDecks: boolean
  /** The fix-mark endpoints; with them the card gets a flag that marks it important to fix. */
  fixMarks?: FixMarksApi
}

/** One card the game does not run whole: its picture, name and state, the decks that hold it and each unread clause. */
export function UnvalidatedCard({ card, faces, pending, showDecks, fixMarks }: UnvalidatedCardProps) {
  return (
    <li
      data-card={card.oracleId}
      data-support={card.support}
      className={cn('surface flex gap-4 border-l-4 p-4', supportEdge(card.support))}
    >
      <CardPicture oracleId={card.oracleId} name={card.name} size="small" alt="" className="w-16 sm:w-24" />
      <div className="grid min-w-0 flex-1 content-start gap-2">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <p className="min-w-0 text-base font-semibold break-words">{card.name}</p>
          <SupportMark support={card.support} />
          {fixMarks === undefined ? null : (
            <FixMarkButton oracleId={card.oracleId} name={card.name} api={fixMarks} emphasis="prominent" />
          )}
        </div>
        {showDecks ? (
          <p className="text-sm text-muted-foreground" data-slot="decks">
            {`In ${card.decks.length} ${card.decks.length === 1 ? 'deck' : 'decks'}: `}
            {card.decks.map((deck, index) => (
              <span key={deck.id}>
                {index === 0 ? '' : ', '}
                <Link
                  to="/decks/$deckId"
                  params={{ deckId: deck.id }}
                  className="text-foreground underline-offset-4 hover:underline"
                >
                  {deck.name}
                </Link>
              </span>
            ))}
          </p>
        ) : null}
        <ClauseList lines={clauseLines(card, faces)} support={card.support} pending={pending} />
        <DraftNote draft={card.draft} />
      </div>
    </li>
  )
}
