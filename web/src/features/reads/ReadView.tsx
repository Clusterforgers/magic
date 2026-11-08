import { Link } from '@tanstack/react-router'
import { BookOpenIcon, CheckIcon, ChevronLeftIcon, ChevronRightIcon, TriangleAlertIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { describeDeckError } from '@/features/decks/errors'
import { cn } from '@/lib/utils'
import { BY_HAND } from '@/lib/byHand'
import { TO_READ } from '@/lib/toRead'
import type { ReadTarget } from '@/lib/toRead'
import type { ReadsApi } from '@/net/reads'
import type { ReadQueue, ReadVerdict } from '@/protocol/generated'
import { STATE_WORDS, nextToRead } from './model'
import { ReadCardView } from './ReadCardView'

type QueueLoad =
  { status: 'loading' } | { status: 'ready'; queue: ReadQueue } | { status: 'error'; error: unknown }

function useQueue(api: Pick<ReadsApi, 'queue'>, target: ReadTarget) {
  const [load, setLoad] = useState<QueueLoad>({ status: 'loading' })
  const key = 'deck' in target ? `deck:${target.deck}` : `precon:${target.precon}`
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

/** The way back to the deck the cards are read from. */
function BackLink({ target }: { target: ReadTarget }) {
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
  target: ReadTarget
}

/**
 * The review screen's body (docs/web.md, Reading cards): the deck's waiting cards in a list (from `lg`; a "Card 2 of 7"
 * bar with Previous and Next under it) beside the card being read. A verdict moves on to the next card not judged.
 */
export function ReadView({ api, target }: ReadViewProps) {
  const { load, retry } = useQueue(api, target)
  const [current, setCurrent] = useState<string | null>(null)
  const [done, setDone] = useState<ReadonlyMap<string, ReadVerdict>>(new Map())
  const [said, setSaid] = useState('')
  const cards = load.status === 'ready' ? load.queue.cards : []
  const shown = current ?? cards[0]?.oracle_id ?? null

  const judged = (oracleId: string, name: string, verdict: ReadVerdict): void => {
    const next = new Map(done).set(oracleId, verdict)
    setDone(next)
    const following = nextToRead(cards, new Set(next.keys()), oracleId)
    const nextName = cards.find((card) => card.oracle_id === following)?.name
    setSaid(
      `${name}: ${verdict === 'correct' ? 'correct' : 'wrong'}, saved.${nextName === undefined ? '' : ` Next: ${nextName}.`}`,
    )
    if (following !== null) setCurrent(following)
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
  return (
    <section aria-labelledby="read-title" className="grid gap-4">
      <header className="flex flex-wrap items-end justify-between gap-3">
        <div className="grid min-w-0 gap-1">
          <h1 id="read-title" className="text-2xl font-semibold tracking-tight break-words">
            Read the cards of {queue.title}
          </h1>
          <p className="text-sm text-muted-foreground">
            {cards.length === 0
              ? 'Nothing waits for your read in this deck.'
              : `${cards.length - left} of ${cards.length} read. Read each clause beside the game's own words, then say whether the game does what the card says.`}
          </p>
        </div>
        {cards.length === 0 || finished ? null : <BackLink target={target} />}
      </header>
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
            ? `Thank you. A card found correct runs automatically from the next game; one found wrong is ${BY_HAND} until it is fixed.`
            : 'Every draft in this deck has your read, or the deck has none.'}
        </EmptyState>
      ) : (
        <div className="grid gap-4 lg:grid-cols-[18rem_minmax(0,1fr)] lg:items-start">
          <nav aria-label={`Cards ${TO_READ}`} className="hidden lg:block">
            <ul className="surface grid gap-0.5 p-1">
              {cards.map((card) => {
                const verdict = done.get(card.oracle_id)
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
                        <span className="min-w-0 truncate">{card.name}</span>
                      </span>
                      <span className="flex items-center gap-1 text-xs text-muted-foreground">
                        {verdict === undefined ? STATE_WORDS[card.state] : `You said ${verdict}`}
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
                  api={api}
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
