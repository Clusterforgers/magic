import { CheckIcon } from 'lucide-react'
import { useContext, useEffect, useRef } from 'react'
import { Kbd } from '@/components/Kbd'
import { Skeleton } from '@/components/ui/skeleton'
import { Button } from '@/components/ui/button'
import { useCardDetail } from '@/features/builder/useCardDetail'
import { CardPicture, HoverPicture } from '@/features/cards/CardPicture'
import type { ByHandApi } from '@/features/decks/byHand'
import { PlayByHand } from '@/features/decks/PlayByHand'
import { LineMarksList } from '@/features/decks/preview/LineMarks'
import { SupportMark } from '@/features/decks/SupportMark'
import { ClauseList } from '@/features/decks/unvalidated/ClauseList'
import { clauseLines } from '@/features/decks/unvalidated/model'
import type { FlaggedCard } from '@/features/decks/unvalidated/model'
import { CardsApiContext } from '@/features/table/cardsContext'
import { BY_HAND } from '@/lib/byHand'
import { cardWords } from '@/lib/lineMarks'
import { reviewKey } from './review'
import { verdictKey } from './model'

interface CheckCardViewProps {
  card: FlaggedCard
  /** The deck endpoints that mark the card "play by hand"; none where the decks are not the reader's (a precon). */
  byHand?: ByHandApi
  /** The reader marked the card checked already. */
  checked: boolean
  /** The reader says the card is checked. */
  onChecked: () => void
  /** Whether C and H act: off while a dialog has the keys. */
  keys?: boolean
}

/** The card's Oracle text with each line marked, and the reasons of the lines played by hand. */
function OracleLines({ card }: { card: FlaggedCard }) {
  const known = !card.oracleId.startsWith('name:')
  const { state } = useCardDetail(useContext(CardsApiContext), known ? card.oracleId : null)
  const faces =
    state.status === 'ready'
      ? state.detail.faces.length > 0
        ? state.detail.faces.map((face) => face.oracle_text)
        : [state.detail.oracle_text]
      : undefined
  return (
    <>
      <section aria-labelledby={`oracle-${card.oracleId}`} className="grid gap-2">
        <h3 id={`oracle-${card.oracleId}`} className="text-sm font-medium text-muted-foreground">
          Oracle text
        </h3>
        {state.status === 'ready' ? (
          <div className="rounded-lg border border-border bg-raised p-3">
            <LineMarksList detail={state.detail} />
          </div>
        ) : state.status === 'loading' && known ? (
          <Skeleton className="h-20 w-full" />
        ) : (
          <p className="text-sm text-muted-foreground">The card&apos;s text could not be read.</p>
        )}
      </section>
      <section aria-labelledby={`why-${card.oracleId}`} className="grid gap-2">
        <h3 id={`why-${card.oracleId}`} className="text-sm font-medium text-muted-foreground">
          What you do by hand
        </h3>
        <ClauseList
          lines={clauseLines(card, faces)}
          support={card.support}
          pending={state.status === 'loading' && known}
        />
      </section>
    </>
  )
}

/**
 * One card to check (docs/web.md, Reviewing cards): its picture beside the Oracle text with its lines marked, what the
 * players do by hand and why, "Play by hand in <deck>" for each of the reader's decks, and Checked.
 */
export function CheckCardView({ card, byHand, checked, onChecked, keys = true }: CheckCardViewProps) {
  const root = useRef<HTMLElement>(null)
  useEffect(() => {
    if (!keys) return
    const onKey = (event: KeyboardEvent): void => {
      if (verdictKey(event) === 'correct') {
        event.preventDefault()
        onChecked()
        return
      }
      if (reviewKey(event) !== 'by_hand') return
      const buttons = root.current?.querySelectorAll<HTMLButtonElement>('[data-slot="play-by-hand"]') ?? []
      const only = buttons.length === 1 ? buttons[0] : undefined
      if (only === undefined || only.disabled) return
      event.preventDefault()
      only.click()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [keys, onChecked])

  const decks = byHand === undefined || card.support === 'unplayable' ? [] : card.decks
  return (
    <article
      ref={root}
      data-slot="check-card"
      data-oracle-id={card.oracleId}
      data-support={card.support}
      className="grid gap-4 md:grid-cols-[minmax(0,15rem)_minmax(0,1fr)] md:items-start md:gap-6"
    >
      <div data-slot="review-picture" className="mx-auto w-32 md:sticky md:top-4 md:w-full">
        <HoverPicture pin oracleId={card.oracleId} name={card.name} className="w-full">
          <CardPicture
            oracleId={card.oracleId}
            name={card.name}
            size="normal"
            alt=""
            eager
            className="w-full"
          />
        </HoverPicture>
      </div>
      <div className="grid min-w-0 gap-4">
        <header className="grid gap-1">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="text-xl font-semibold tracking-tight break-words">{card.name}</h2>
            <SupportMark support={card.support} />
          </div>
          <p className="text-sm text-muted-foreground">
            {`${cardWords(card.support)}. Check what the game leaves to you, and mark it ${BY_HAND} where it misbehaves.`}
          </p>
        </header>
        <OracleLines card={card} />
        <div
          data-slot="check-actions"
          className="sticky bottom-0 z-10 -mb-4 grid gap-3 border-t border-border bg-surface pt-3 pb-4"
        >
          {byHand === undefined || decks.length === 0 ? null : (
            <PlayByHand
              oracleId={card.oracleId}
              name={card.name}
              api={byHand}
              decks={decks.map((deck) => ({ ...deck, byHand: card.byHandIn?.includes(deck.id) ?? false }))}
            />
          )}
          <div className="flex flex-wrap items-center gap-2">
            <Button data-slot="check-done" onClick={onChecked} aria-pressed={checked}>
              <CheckIcon aria-hidden="true" />
              {checked ? 'Checked' : 'Mark checked'}
              <Kbd aria-hidden="true">C</Kbd>
            </Button>
          </div>
        </div>
      </div>
    </article>
  )
}
