import { Link } from '@tanstack/react-router'
import {
  BookOpenIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  HandIcon,
  KeyboardIcon,
  ListIcon,
} from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Kbd } from '@/components/Kbd'
import { Loading } from '@/components/Loading'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { describeDeckError } from '@/features/decks/errors'
import { PlayByHand } from '@/features/decks/PlayByHand'
import type { ByHandApi } from '@/features/decks/byHand'
import { BY_HAND } from '@/lib/byHand'
import type { ReadTarget } from '@/lib/toRead'
import { bulkReadsApi } from '@/net/bulkReads'
import type { BulkReadsApi } from '@/net/bulkReads'
import { decksApi } from '@/net/decks'
import { preconsApi } from '@/net/precons'
import type { ReadsApi } from '@/net/reads'
import type { ReadCard, ReadQueue, ReadVerdict } from '@/protocol/generated'
import { BulkApprove } from './BulkApprove'
import { CheckCardView } from './CheckCardView'
import { countLine, queueKey, queueWords, savePosition, savedPosition } from './model'
import { ReadCardView } from './ReadCardView'
import { nextOpen, progressOf, reviewItems, reviewKey, reviewLine, saveChecked, savedChecked } from './review'
import type { Outcome } from './review'
import { ReviewKeys, ReviewList } from './ReviewList'
import { useCheckCards } from './useCheckCards'
import type { CheckApis } from './useCheckCards'

const DEFAULT_CHECKS: CheckApis = { decks: decksApi, precons: preconsApi }

/** The verdict and the check actions stay in view at the foot of the card while its rules scroll. */
const STICKY_BAR = 'sticky bottom-0 z-10 -mb-4 border-t border-border bg-surface pt-3 pb-4'

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
      <Link to="/decks" className={buttonVariants({ variant: 'outline' })}>
        Your decks
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

/** The list of the same cards with filters and Copy as text, for the reader's own decks; none for a precon. */
function ListLink({ target }: { target: ReadTarget }) {
  if ('precon' in target) return null
  const look = buttonVariants({ variant: 'ghost' })
  const body = (
    <>
      <ListIcon aria-hidden="true" />
      See as a list
    </>
  )
  return 'deck' in target ? (
    <Link to="/decks/$deckId/unvalidated" params={{ deckId: target.deck }} className={look}>
      {body}
    </Link>
  ) : (
    <Link to="/decks/unvalidated" className={look}>
      {body}
    </Link>
  )
}

interface ReadViewProps {
  api: ReadsApi
  /** The deck whose cards are reviewed; null for every card waiting for a first read. */
  target: ReadTarget | null
  /** The endpoints that mark a card "play by hand" in a deck; with them a card found wrong offers that. */
  decks?: ByHandApi
  /** The endpoints the cards to check come from; the shared clients unless a test brings its own. */
  checks?: CheckApis
  /** The bulk read endpoints; the shared client unless a test brings its own. */
  bulk?: BulkReadsApi
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

/** Done so far: a bar and its words, "5 of 12 reviewed". */
function Progress({ done, total, text }: { done: number; total: number; text: string }) {
  const share = total === 0 ? 0 : Math.round((done / total) * 100)
  return (
    <div className="grid w-full max-w-xs gap-1">
      <div
        role="progressbar"
        aria-label="Review progress"
        aria-valuemin={0}
        aria-valuemax={total}
        aria-valuenow={done}
        aria-valuetext={text}
        className="h-2 overflow-hidden rounded-full bg-muted"
      >
        <div className="h-full rounded-full bg-primary transition-[width]" style={{ width: `${share}%` }} />
      </div>
      <p data-slot="review-progress" className="text-xs text-muted-foreground tabular-nums">
        {text}
      </p>
    </div>
  )
}

/**
 * The review screen's body (docs/web.md, Reviewing cards): the cards to read, most played first, then the cards to
 * check, in a list (from `lg`; a "Card 2 of 7" bar with Previous and Next under it) beside the card on show. A verdict
 * or Checked opens the next open card at once, loaded ahead; the browser keeps the card the reader is on and the cards
 * checked, so a reload opens the queue there.
 */
export function ReadView({
  api,
  target,
  decks = decksApi,
  checks = DEFAULT_CHECKS,
  bulk = bulkReadsApi,
}: ReadViewProps) {
  const { load, retry } = useQueue(api, target)
  const checkCards = useCheckCards(checks, target)
  const { client, prefetch } = usePrefetching(api)
  const key = queueKey(target)
  const [current, setCurrent] = useState<string | null>(null)
  const [done, setDone] = useState<ReadonlyMap<string, Outcome>>(
    () => new Map(savedChecked(key).map((id) => [id, 'checked'])),
  )
  const [said, setSaid] = useState('')
  const [offer, setOffer] = useState<WrongOffer | null>(null)
  const [keysShown, setKeysShown] = useState(false)
  const [bulkOpen, setBulkOpen] = useState(false)
  const reads = load.status === 'ready' ? load.queue.cards : null
  const flagged = checkCards.status === 'ready' ? checkCards.cards : null
  const items = useMemo(() => reviewItems(reads ?? [], flagged ?? []), [reads, flagged])
  const kept = useMemo(() => savedPosition(key), [key])
  const resumed = items.some((item) => item.id === kept) ? kept : null
  const shown = current ?? resumed ?? items[0]?.id ?? null
  const following = shown === null ? null : nextOpen(items, done, shown)
  const followingItem = items.find((item) => item.id === following)
  const openReads = useMemo(
    () => new Set(items.filter((item) => item.kind === 'read' && !done.has(item.id)).map((item) => item.id)),
    [items, done],
  )
  const listRef = useRef<HTMLDivElement>(null)
  const index = items.findIndex((item) => item.id === shown)

  useEffect(() => {
    if (shown !== null) savePosition(key, shown)
  }, [key, shown])
  useEffect(() => {
    if (followingItem?.kind === 'read' && followingItem.id !== shown) prefetch(followingItem.id)
  }, [followingItem, shown, prefetch])
  useEffect(() => {
    listRef.current?.querySelector('[aria-current="true"]')?.scrollIntoView?.({ block: 'nearest' })
  }, [shown])

  const step = (by: number): void => {
    const item = items[(index + by + items.length) % items.length]
    if (item !== undefined) setCurrent(item.id)
  }
  const stepRef = useRef(step)
  useEffect(() => {
    stepRef.current = step
  })
  useEffect(() => {
    if (bulkOpen) return
    const onKey = (event: KeyboardEvent): void => {
      const asked = reviewKey(event)
      if (asked === 'next' || asked === 'previous') {
        event.preventDefault()
        stepRef.current(asked === 'next' ? 1 : -1)
      } else if (asked === 'help') {
        event.preventDefault()
        setKeysShown((was) => !was)
      }
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [bulkOpen])

  const settle = (next: Map<string, Outcome>, id: string, words: string): void => {
    setDone(next)
    const after = nextOpen(items, next, id)
    const nextName = items.find((item) => item.id === after)?.name
    setSaid(`${words}${nextName === undefined ? '' : ` Next: ${nextName}.`}`)
    if (after !== null) setCurrent(after)
  }
  const judged = (oracleId: string, name: string, verdict: ReadVerdict): void => {
    const item = items.find((entry) => entry.id === oracleId)
    const inDecks = item?.kind === 'read' ? item.card.in_decks : false
    setOffer(verdict === 'wrong' ? { oracleId, name, inDecks } : null)
    settle(
      new Map(done).set(oracleId, verdict),
      oracleId,
      `${name}: ${verdict === 'correct' ? 'reads right' : 'reads wrong'}, saved.`,
    )
  }
  const checked = (id: string, name: string): void => {
    const next = new Map(done).set(id, 'checked')
    saveChecked(
      key,
      [...next].filter(([, outcome]) => outcome === 'checked').map(([checkedId]) => checkedId),
    )
    setOffer(null)
    settle(next, id, `${name}: checked.`)
  }
  const approved = (ids: readonly string[]): void => {
    const next = new Map(done)
    for (const id of ids) next.set(id, 'correct')
    setDone(next)
    if (shown !== null && ids.includes(shown)) {
      const after = nextOpen(items, next, shown)
      if (after !== null) setCurrent(after)
    }
  }

  if (load.status === 'loading') {
    return (
      <Loading label="Loading the cards to review…">
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
          Review cards
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
  const progress = progressOf(items, done)
  const finished = items.length > 0 && progress.done === progress.total
  const words = queueWords(target, queue.title)
  const shownItem = index < 0 ? undefined : items[index]
  const ownDecks = target !== null && !('precon' in target)
  const checksLeft = items.filter((item) => item.kind === 'check' && !done.has(item.id)).length
  return (
    <section aria-labelledby="read-title" className="grid gap-4">
      <header className="flex flex-wrap items-end justify-between gap-x-6 gap-y-3">
        <div className="grid min-w-0 flex-[1_1_20rem] gap-2">
          <h1 id="read-title" className="text-2xl font-semibold tracking-tight break-words">
            {words.heading}
          </h1>
          {items.length === 0 ? (
            <p className="text-sm text-muted-foreground">{words.nothing}</p>
          ) : (
            <>
              <p className="text-sm text-muted-foreground">
                <span data-slot="read-count" className="font-medium text-foreground tabular-nums">
                  {reviewLine(countLine(reads ?? [], new Set(done.keys()), target), checksLeft)}
                </span>{' '}
                Read each card beside the game&apos;s words and say whether it reads right; check what the
                game leaves to you.
              </p>
              <Progress {...progress} />
            </>
          )}
        </div>
        <div className="flex flex-wrap gap-2">
          {items.length === 0 || finished ? null : (
            <>
              <Button
                variant="ghost"
                className="max-md:hidden"
                aria-expanded={keysShown}
                aria-controls="review-keys"
                onClick={() => setKeysShown((was) => !was)}
              >
                <KeyboardIcon aria-hidden="true" />
                Keys
                <Kbd aria-hidden="true">?</Kbd>
              </Button>
              {target === null ? null : <ListLink target={target} />}
              <BackLink target={target} />
            </>
          )}
        </div>
      </header>
      {keysShown && !finished && items.length > 0 ? (
        <div className="surface p-3">
          <ReviewKeys id="review-keys" />
        </div>
      ) : null}
      {finished ? null : (
        <BulkApprove api={bulk} open={openReads} onApproved={approved} onOpenChange={setBulkOpen} />
      )}
      {offer === null ? null : (
        <WrongOfferNote
          offer={offer}
          target={target}
          deckName={queue.title}
          decks={decks}
          onClose={() => setOffer(null)}
        />
      )}
      {checkCards.status === 'failed' ? (
        <p data-slot="check-failed" className="text-sm text-muted-foreground">
          {`The cards ${BY_HAND} could not be listed: ${checkCards.message}`}
        </p>
      ) : null}
      <p aria-live="polite" className="sr-only">
        {said}
      </p>
      {items.length === 0 && checkCards.status === 'loading' ? (
        <Loading label={`Looking for cards ${BY_HAND}…`}>
          <Skeleton className="h-40 w-full" />
        </Loading>
      ) : items.length === 0 || finished ? (
        <EmptyState
          icon={BookOpenIcon}
          title={finished ? 'Every card is reviewed' : 'Nothing to review'}
          action={<BackLink target={target} />}
        >
          {finished
            ? `Thank you. A card that reads right counts as read from the next game; one that reads wrong is ${BY_HAND} until it is fixed.`
            : words.empty}
        </EmptyState>
      ) : (
        <div className="grid gap-4 lg:grid-cols-[18rem_minmax(0,1fr)] lg:items-start">
          <div className="hidden min-w-0 grid-cols-1 gap-2 lg:sticky lg:top-4 lg:grid">
            <ReviewList
              items={items}
              done={done}
              shown={shown}
              onPick={setCurrent}
              target={target}
              listRef={listRef}
            />
            {checkCards.status === 'loading' ? (
              <p role="status" className="px-2 text-xs text-muted-foreground">
                {`Looking for cards ${BY_HAND}…`}
              </p>
            ) : null}
          </div>
          <div className="grid min-w-0 gap-4">
            <div className="surface flex items-center justify-between gap-2 p-2 lg:hidden">
              <Button variant="ghost" size="sm" onClick={() => step(-1)} aria-label="Previous card">
                <ChevronLeftIcon aria-hidden="true" />
                Previous
              </Button>
              <p className="text-sm tabular-nums">
                Card {index + 1} of {items.length}
              </p>
              <Button variant="ghost" size="sm" onClick={() => step(1)} aria-label="Next card">
                Next
                <ChevronRightIcon aria-hidden="true" />
              </Button>
            </div>
            {shownItem === undefined ? null : (
              <div className="surface p-4">
                {shownItem.kind === 'read' ? (
                  <ReadCardView
                    key={shownItem.id}
                    api={client}
                    oracleId={shownItem.id}
                    keys={!bulkOpen}
                    verdictClassName={STICKY_BAR}
                    onJudged={(card, verdict) => judged(card.oracle_id, card.name, verdict)}
                  />
                ) : (
                  <CheckCardView
                    key={shownItem.id}
                    card={shownItem.card}
                    keys={!bulkOpen}
                    checked={done.get(shownItem.id) === 'checked'}
                    onChecked={() => checked(shownItem.id, shownItem.name)}
                    {...(ownDecks ? { byHand: decks } : {})}
                  />
                )}
              </div>
            )}
          </div>
        </div>
      )}
    </section>
  )
}
