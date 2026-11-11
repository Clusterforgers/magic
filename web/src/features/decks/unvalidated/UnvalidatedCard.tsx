import { Link } from '@tanstack/react-router'
import { CardPicture, HoverPicture } from '@/features/cards/CardPicture'
import { FixMarkButton } from '@/features/fixmarks/FixMarkButton'
import { cn } from '@/lib/utils'
import type { FixMarksApi } from '@/net/fixMarks'
import { DraftNote } from '../DraftNote'
import { PlayByHand } from '../PlayByHand'
import type { ByHandApi } from '../byHand'
import { SupportMark, supportEdge } from '../SupportMark'
import { ClauseList } from './ClauseList'
import { DiffersList } from './DiffersList'
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
  /** The endpoints that mark the card "play by hand" in a deck; with them it offers that for each of its decks. */
  byHand?: ByHandApi
}

/** One card the game does not run whole: its picture, name and state, the decks that hold it and each unread clause. */
export function UnvalidatedCard({ card, faces, pending, showDecks, fixMarks, byHand }: UnvalidatedCardProps) {
  return (
    <li
      data-card={card.oracleId}
      data-support={card.support}
      className={cn('surface flex gap-4 border-l-4 p-4', supportEdge(card.support))}
    >
      <HoverPicture
        pin
        oracleId={card.oracleId}
        name={card.name}
        info={{ support: card.support }}
        className="shrink-0 self-start"
      >
        <CardPicture oracleId={card.oracleId} name={card.name} size="small" alt="" className="w-16 sm:w-24" />
      </HoverPicture>
      <div className="grid min-w-0 flex-1 content-start gap-2">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <p className="min-w-0 text-base font-semibold break-words">{card.name}</p>
          <SupportMark support={card.support} />
          {fixMarks === undefined ? null : (
            <FixMarkButton
              oracleId={card.oracleId}
              name={card.name}
              api={fixMarks}
              emphasis="prominent"
              support={card.support}
            />
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
        {byHand === undefined || card.support === 'unplayable' ? null : (
          <PlayByHand
            oracleId={card.oracleId}
            name={card.name}
            api={byHand}
            decks={card.decks.map((deck) => ({ ...deck, byHand: card.byHandIn?.includes(deck.id) ?? false }))}
          />
        )}
        <DiffersList differs={card.differs ?? []} />
        <ClauseList lines={clauseLines(card, faces)} support={card.support} pending={pending} />
        <DraftNote draft={card.draft} />
      </div>
    </li>
  )
}
