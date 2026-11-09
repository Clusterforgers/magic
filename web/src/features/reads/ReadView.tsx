import { Link } from '@tanstack/react-router'
import {
  BookOpenIcon,
  CheckIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  HandIcon,
  LibraryIcon,
  TriangleAlertIcon,
} from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { BulkLink } from '@/features/bulk-reads/BulkLink'
import { describeDeckError } from '@/features/decks/errors'
import { PlayByHand } from '@/features/decks/PlayByHand'
import type { ByHandApi } from '@/features/decks/byHand'
import { cn } from '@/lib/utils'
import { BY_HAND } from '@/lib/byHand'
import { TO_READ } from '@/lib/toRead'
import type { ReadTarget } from '@/lib/toRead'
import { decksApi } from '@/net/decks'
import type { ReadsApi } from '@/net/reads'
import type { ReadCard, ReadQueue, ReadVerdict } from '@/protocol/generated'
import {
  STATE_WORDS,
  allInDecks,
  countLine,
  nextToRead,
  queueKey,
  queueWords,
  rankText,
  savePosition,
  savedPosition,
} from './model'
import { ReadCardView } from './ReadCardView'

type QueueLoad =
  { status: 'loading' } | { status: 'ready'; queue: ReadQueue } | { status: 'error'; error: unknown }

function useQueue(api: Pick<ReadsApi, 'queue'>, target: ReadTarget | null) {
  const [load, setLoad] = useState<QueueLoad>({ status: 'loading' })
  const key = queueKey(target)
  const [attempt, setAttempt] = useState(0)
  useEffect(() => {
    let live = true
    setLoad({ status: 'loading' })
    api.queue(target).then(
      (queue) => {
        if (live) setLoad({ status: 'ready', queue })
      },
      (error: unknown) => {
        if (live) setLoad({ status: 'error', error })
      },
    )
    return () => {
      live = false
    }
    // `key` stands for the target, which a caller may rebuild on every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api, key, attempt])
  return { load, retry: () => setAttempt((n) => n + 1) }
}

/**
 * The read client with one card asked for ahead: `prefetch` starts loading the card that comes next, and the next
 * `get` of it takes that answer, so a verdict opens the next card at once.
 */
function usePrefetching(api: ReadsApi) {
  return useMemo(() => {
    const ahead = new Map<string, Promise<ReadCard>>()
    const client: ReadsApi = {
      ...api,
      get: (oracleId) => {
        const early = ahead.get(oracleId)
        ahead.delete(oracleId)
        return early ?? api.get(oracleId)
      },
    }
    const prefetch = (oracleId: string): void => {
      if (ahead.has(oracleId)) return
      const early = api.get(oracleId)
      // A failed fetch ahead is asked again when the card opens.
      early.catch(() => ahead.delete(oracleId))
      ahead.clear()
      ahead.set(oracleId, early)
    }
    return { client, prefetch }
  }, [api])
}

/** The way back to the deck the cards are read from, or to the decks for every card. */
function BackLink({ target }: { target: ReadTarget | null }) {
  if (target === null) {
    return (
      <Link to="/decks" className={buttonVariants({ variant: 'outline' })}>
        Your decks
      </Link>
    )
  }
  if ('mine' in target) {
    return (
      <Link to="/decks/unvalidated" className={buttonVariants({ variant: 'outline' })}>
        Cards to check
      </Link>
    )
  }
  return 'deck' in target ? (
    <Link
      to="/decks/$deckId"
      params={{ deckId: target.deck }}
      className={buttonVariants({ variant: 'outline' })}
    >
      Back to the deck
    </Link>
  ) : (
    <Link to="/precons" className={buttonVariants({ variant: 'outline' })}>
      Back to the precons
    </Link>
  )
}

interface ReadViewProps {
  api: ReadsApi
  /** The deck whose cards are read; null for every card waiting for a first read. */
  target: ReadTarget | null
  /** The endpoints that mark a card "play by hand" in a deck; with them a card found wrong offers that. */
  decks?: ByHandApi
}

/** A card the reader just found wrong, offered to be played by hand. */
interface WrongOffer {
  oracleId: string
  name: string
  inDecks: boolean
}

/**
 * After a Wrong verdict: the offer to play the card by hand, in the deck being read (a button that marks it there) or,
 * for a card of the reader's decks read from elsewhere, a link to the not-validated list that offers it per deck.
 */
function WrongOfferNote({
  offer,
  target,
  deckName,
  decks,
  onClose,
}: {
  offer: WrongOffer
  target: ReadTarget | null
  deckName: string
  decks: ByHandApi
  onClose: () => void
}) {
  const inDeck = target !== null && 'deck' in target
  if (!inDeck && !offer.inDecks) return null
  return (
    <Alert data-slot="wrong-offer">
      <HandIcon aria-hidden="true" />
      <AlertTitle>{`${offer.name} is found wrong`}</AlertTitle>
      <AlertDescription className="grid justify-items-start gap-2">
        <span>{`Mark it to keep it ${BY_HAND} at your table, even once its rules change.`}</span>
        <div className="flex flex-wrap items-center gap-3">
          {inDeck ? (
            <PlayByHand
              oracleId={offer.oracleId}
              name={offer.name}
              api={decks}
              decks={[{ id: target.deck, name: deckName, byHand: false }]}
            />
          ) : (
            <Link to="/decks/unvalidated" className={buttonVariants({ variant: 'outline', size: 'xs' })}>
              Choose the decks to play it by hand in
            </Link>
          )}
          <Button type="button" variant="ghost" size="xs" onClick={onClose}>
            Dismiss
          </Button>
        </div>
      </AlertDescription>
    </Alert>
  )
}

/**
 * The review screen's body (docs/web.md, Reading cards): the waiting cards, most played first, in a list (from `lg`;
 * a "Card 2 of 7" bar with Previous and Next under it) beside the card being read. A verdict opens the next card not
 * judged at once, loaded ahead; the browser keeps the card the reader is on, so a reload opens the queue there.
 */
export function ReadView({ api, target, decks = decksApi }: ReadViewProps) {
  const { load, retry } = useQueue(api, target)
  const { client, prefetch } = usePrefetching(api)
  const key = queueKey(target)
  const [current, setCurrent] = useState<string | null>(null)
  const [done, setDone] = useState<ReadonlyMap<string, ReadVerdict>>(new Map())
  const [said, setSaid] = useState('')
  const [offer, setOffer] = useState<WrongOffer | null>(null)
  const cards = load.status === 'ready' ? load.queue.cards : []
  const kept = useMemo(() => savedPosition(key), [key])
  const resumed = cards.some((card) => card.oracle_id === kept) ? kept : null
  const shown = current ?? resumed ?? cards[0]?.oracle_id ?? null
  const following = shown === null ? null : nextToRead(cards, new Set(done.keys()), shown)
  const listRef = useRef<HTMLUListElement>(null)

  useEffect(() => {
    if (shown !== null) savePosition(key, shown)
  }, [key, shown])
  useEffect(() => {
    if (following !== null && following !== shown) prefetch(following)
  }, [following, shown, prefetch])
  useEffect(() => {
    listRef.current?.querySelector('[aria-current="true"]')?.scrollIntoView?.({ block: 'nearest' })
  }, [shown])

  const judged = (oracleId: string, name: string, verdict: ReadVerdict): void => {
    const next = new Map(done).set(oracleId, verdict)
    const inDecks = cards.find((card) => card.oracle_id === oracleId)?.in_decks ?? false
    setOffer(verdict === 'wrong' ? { oracleId, name, inDecks } : null)
    setDone(next)
    const after = nextToRead(cards, new Set(next.keys()), oracleId)
    const nextName = cards.find((card) => card.oracle_id === after)?.name
    setSaid(
      `${name}: ${verdict === 'correct' ? 'correct' : 'wrong'}, saved.${nextName === undefined ? '' : ` Next: ${nextName}.`}`,
    )
    if (after !== null) setCurrent(after)
  }

  if (load.status === 'loading') {
    return (
      <Loading label={`Loading the cards ${TO_READ}…`}>
        <div className="grid gap-3">
          <Skeleton className="h-8 w-64" />
          <Skeleton className="h-40 w-full" />
        </div>
      </Loading>
    )
  }
  if (load.status === 'error') {
    return (
      <section aria-labelledby="read-title" className="grid justify-items-start gap-3">
        <h1 id="read-title" className="text-2xl font-semibold tracking-tight">
          Read cards
        </h1>
        <Alert variant="destructive">
          <AlertTitle>The cards could not be loaded</AlertTitle>
          <AlertDescription className="grid justify-items-start gap-2">
            <span>{describeDeckError(load.error)}</span>
            <Button variant="outline" size="sm" onClick={retry}>
              Try again
            </Button>
          </AlertDescription>
        </Alert>
      </section>
    )
  }

  const { queue } = load
  const index = cards.findIndex((card) => card.oracle_id === shown)
  const left = cards.filter((card) => !done.has(card.oracle_id)).length
  const finished = cards.length > 0 && left === 0
  const step = (by: number): void => {
    const card = cards[(index + by + cards.length) % cards.length]
    if (card !== undefined) setCurrent(card.oracle_id)
  }
  const words = queueWords(target, queue.title)
  return (
    <section aria-labelledby="read-title" className="grid gap-4">
      <header className="flex flex-wrap items-end justify-between gap-3">
        <div className="grid min-w-0 gap-1">
          <h1 id="read-title" className="text-2xl font-semibold tracking-tight break-words">
            {words.heading}
          </h1>
          <p className="text-sm text-muted-foreground">
            {cards.length === 0 ? (
              words.nothing
            ) : (
              <>
                <span data-slot="read-count" className="font-medium text-foreground tabular-nums">
                  {countLine(cards, new Set(done.keys()), target)}
                </span>{' '}
                Most played first. Read each clause beside the game&apos;s own words, then say whether the
                game does what the card says.
              </>
            )}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <BulkLink />
          {cards.length === 0 || finished ? null : (
            <>
              {target === null ? null : (
                <Link to="/read" className={buttonVariants({ variant: 'ghost' })}>
                  <LibraryIcon aria-hidden="true" />
                  Every card
                </Link>
              )}
              <BackLink target={target} />
            </>
          )}
        </div>
      </header>
      {offer === null ? null : (
        <WrongOfferNote
          offer={offer}
          target={target}
          deckName={queue.title}
          decks={decks}
          onClose={() => setOffer(null)}
        />
      )}
      <p aria-live="polite" className="sr-only">
        {said}
      </p>
      {cards.length === 0 || finished ? (
        <EmptyState
          icon={BookOpenIcon}
          title={finished ? 'Every card is read' : `No cards ${TO_READ}`}
          action={<BackLink target={target} />}
        >
          {finished
            ? `Thank you. A card found correct counts as read from the next game; one found wrong is ${BY_HAND} until it is fixed.`
            : words.empty}
        </EmptyState>
      ) : (
        <div className="grid gap-4 lg:grid-cols-[18rem_minmax(0,1fr)] lg:items-start">
          <nav aria-label={`Cards ${TO_READ}`} className="hidden lg:sticky lg:top-4 lg:block">
            <ul
              ref={listRef}
              className="surface grid max-h-[calc(100dvh-12rem)] gap-0.5 overflow-y-auto overscroll-contain p-1"
            >
              {cards.map((card) => {
                const verdict = done.get(card.oracle_id)
                const rank = rankText(card.rank)
                return (
                  <li key={card.oracle_id}>
                    <button
                      type="button"
                      aria-current={card.oracle_id === shown ? 'true' : undefined}
                      onClick={() => setCurrent(card.oracle_id)}
                      className={cn(
                        'grid w-full gap-0.5 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent',
                        card.oracle_id === shown ? 'bg-accent font-medium' : null,
                      )}
                    >
                      <span className="flex items-center gap-1.5">
                        {verdict === undefined ? null : (
                          <CheckIcon aria-label="Read" className="size-4 shrink-0 text-support-verified" />
                        )}
                        <span className="min-w-0 flex-1 truncate">{card.name}</span>
                        {rank === null ? null : (
                          <span
                            title="Popularity rank"
                            className="shrink-0 text-xs font-normal text-muted-foreground tabular-nums"
                          >
                            {rank}
                          </span>
                        )}
                      </span>
                      <span className="flex flex-wrap items-center gap-x-1 text-xs text-muted-foreground">
                        {verdict === undefined ? STATE_WORDS[card.state] : `You said ${verdict}`}
                        {card.in_decks && !allInDecks(target) ? <span>· in your decks</span> : null}
                        {card.differs === 0 ? null : (
                          <>
                            <TriangleAlertIcon aria-hidden="true" className="size-3 text-support-partial" />
                            {card.differs === 1 ? '1 clause differs' : `${card.differs} clauses differ`}
                          </>
                        )}
                      </span>
                    </button>
                  </li>
                )
              })}
            </ul>
          </nav>
          <div className="grid min-w-0 gap-4">
            <div className="surface flex items-center justify-between gap-2 p-2 lg:hidden">
              <Button variant="ghost" size="sm" onClick={() => step(-1)} aria-label="Previous card">
                <ChevronLeftIcon aria-hidden="true" />
                Previous
              </Button>
              <p className="text-sm tabular-nums">
                Card {index + 1} of {cards.length}
              </p>
              <Button variant="ghost" size="sm" onClick={() => step(1)} aria-label="Next card">
                Next
                <ChevronRightIcon aria-hidden="true" />
              </Button>
            </div>
            {shown === null ? null : (
              <div className="surface p-4">
                <ReadCardView
                  key={shown}
                  api={client}
                  oracleId={shown}
                  onJudged={(card, verdict) => judged(card.oracle_id, card.name, verdict)}
                />
              </div>
            )}
          </div>
        </div>
      )}
    </section>
  )
}
