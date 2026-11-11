import { Link } from '@tanstack/react-router'
import { CheckCheckIcon } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { EmptyState } from '@/components/EmptyState'
import { Loading } from '@/components/Loading'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button, buttonVariants } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { describeDeckError } from '@/features/decks/errors'
import { forgetToReadCount } from '@/features/reads/useToReadCount'
import type { BulkReadsApi } from '@/net/bulkReads'
import type { BulkCard, BulkReads, BulkRule } from '@/protocol/generated'
import { wordDiff } from './diff'
import type { Piece } from './diff'
import { groupByRule, wordingOnly } from './model'

type Load =
  { status: 'loading' } | { status: 'ready'; reads: BulkReads } | { status: 'error'; error: unknown }

function Marked({ pieces, label }: { pieces: Piece[]; label: string }) {
  return (
    <p className="text-sm break-words">
      <span className="sr-only">{label}: </span>
      {pieces.map((piece, i) =>
        piece.changed ? (
          <span key={i}>
            <mark className="rounded-sm bg-warning/20 px-0.5 text-foreground">{piece.text.trimEnd()}</mark>
            {piece.text.slice(piece.text.trimEnd().length)}
          </span>
        ) : (
          <span key={i}>{piece.text}</span>
        ),
      )}
    </p>
  )
}

/** One card of a bulk approval: its checkbox and name beside each differing clause, the changed words marked. */
export function BulkCardRow({
  card,
  checked,
  onToggle,
}: {
  card: BulkCard
  checked: boolean
  onToggle: () => void
}) {
  const id = `bulk-${card.oracle_id}`
  return (
    <li className="grid gap-2 border-t border-border px-3 py-2 first:border-t-0 md:grid-cols-[1.25rem_12rem_minmax(0,1fr)_minmax(0,1fr)] md:gap-4">
      <div className="flex items-start gap-2 md:contents">
        <input
          id={id}
          type="checkbox"
          checked={checked}
          onChange={onToggle}
          className="mt-0.5 size-4 accent-primary"
        />
        <label htmlFor={id} className="text-sm font-medium break-words">
          {card.name}
        </label>
      </div>
      {card.clauses.map((clause, i) => {
        const diff = wordDiff(clause.printed, clause.engine)
        return (
          <div key={i} className="contents">
            {i > 0 ? <span className="hidden md:col-span-2 md:block" /> : null}
            <div className="grid gap-0.5">
              <span className="text-xs text-muted-foreground md:hidden">Card</span>
              <Marked pieces={diff.left} label="Card" />
            </div>
            <div className="grid gap-0.5">
              <span className="text-xs text-muted-foreground md:hidden">Game</span>
              <Marked pieces={diff.right} label="Game" />
            </div>
          </div>
        )
      })}
    </li>
  )
}

function RuleGroup({
  rule,
  cards,
  chosen,
  onToggle,
  onAll,
}: {
  rule: BulkRule
  cards: BulkCard[]
  chosen: ReadonlySet<string>
  onToggle: (id: string) => void
  onAll: (ids: string[], on: boolean) => void
}) {
  const ids = cards.map((card) => card.oracle_id)
  const all = ids.every((id) => chosen.has(id))
  const headingId = `rule-${rule.name}`
  return (
    <section aria-labelledby={headingId} className="surface overflow-hidden">
      <header className="flex flex-wrap items-center justify-between gap-2 border-b border-border bg-raised px-3 py-2">
        <div className="grid min-w-0 gap-0.5">
          <h2 id={headingId} className="text-base font-semibold">
            {rule.title}{' '}
            <span className="font-normal text-muted-foreground tabular-nums">({cards.length})</span>
          </h2>
          <p className="text-sm text-muted-foreground">{rule.summary}</p>
        </div>
        <Button variant="ghost" size="sm" onClick={() => onAll(ids, !all)}>
          {all ? 'Uncheck all' : 'Check all'}
        </Button>
      </header>
      <div
        aria-hidden="true"
        className="hidden px-3 pt-2 text-xs text-muted-foreground md:grid md:grid-cols-[1.25rem_12rem_minmax(0,1fr)_minmax(0,1fr)] md:gap-4"
      >
        <span />
        <span>Card name</span>
        <span>On the card</span>
        <span>The game says</span>
      </div>
      <ul>
        {cards.map((card) => (
          <BulkCardRow
            key={card.oracle_id}
            card={card}
            checked={chosen.has(card.oracle_id)}
            onToggle={() => onToggle(card.oracle_id)}
          />
        ))}
      </ul>
    </section>
  )
}

/**
 * The bulk read screen's body (docs/web.md, Bulk reads): the drafts whose clauses differ from the card only in wording,
 * grouped by the equivalence they need, every card checked; "Approve N" records one `correct` verdict per checked card.
 */
export function BulkReadView({ api }: { api: BulkReadsApi }) {
  const [load, setLoad] = useState<Load>({ status: 'loading' })
  const [attempt, setAttempt] = useState(0)
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set())
  const [sending, setSending] = useState(false)
  const [said, setSaid] = useState<{ ok: boolean; text: string } | null>(null)

  useEffect(() => {
    let live = true
    api.list().then(
      (reads) => {
        if (!live) return
        setLoad({ status: 'ready', reads })
        setChosen(new Set(reads.cards.map((card) => card.oracle_id)))
      },
      (error: unknown) => {
        if (live) setLoad({ status: 'error', error })
      },
    )
    return () => {
      live = false
    }
  }, [api, attempt])

  const groups = useMemo(() => (load.status === 'ready' ? groupByRule(load.reads) : []), [load])

  if (load.status === 'loading') {
    return (
      <Loading label="Loading the wording-only reads…">
        <div className="grid gap-3">
          <Skeleton className="h-8 w-64" />
          <Skeleton className="h-40 w-full" />
        </div>
      </Loading>
    )
  }
  if (load.status === 'error') {
    return (
      <Alert variant="destructive">
        <AlertTitle>The cards could not be loaded</AlertTitle>
        <AlertDescription className="grid justify-items-start gap-2">
          <span>{describeDeckError(load.error)}</span>
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              setLoad({ status: 'loading' })
              setAttempt((n) => n + 1)
            }}
          >
            Try again
          </Button>
        </AlertDescription>
      </Alert>
    )
  }

  const { cards } = load.reads
  const toggle = (id: string): void => {
    const next = new Set(chosen)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    setChosen(next)
  }
  const setAll = (ids: string[], on: boolean): void => {
    const next = new Set(chosen)
    for (const id of ids) {
      if (on) next.add(id)
      else next.delete(id)
    }
    setChosen(next)
  }
  const approve = (): void => {
    const picked = cards.filter((card) => chosen.has(card.oracle_id))
    setSending(true)
    setSaid(null)
    api
      .approve({ cards: picked.map((card) => ({ oracle_id: card.oracle_id, read_hash: card.read_hash })) })
      .then(
        (result) => {
          const stale = result.stale.length
          setSaid({
            ok: true,
            text: `${wordingOnly(result.approved.length, 'approved')}.${stale === 0 ? '' : ` ${wordingOnly(stale, 'changed since the page loaded and left alone')}.`}`,
          })
          setAttempt((n) => n + 1)
          forgetToReadCount()
        },
        (error: unknown) => setSaid({ ok: false, text: describeDeckError(error) }),
      )
      .finally(() => setSending(false))
  }

  return (
    <section aria-labelledby="bulk-title" className="grid gap-4">
      <header className="flex flex-wrap items-end justify-between gap-3">
        <div className="grid min-w-0 gap-1">
          <h1 id="bulk-title" className="text-2xl font-semibold tracking-tight">
            Wording-only reads
          </h1>
          <p className="text-sm text-muted-foreground">
            The game words these cards differently from the card, and a fixed rule says the two mean the same.
            Glance over each pair, uncheck any that reads wrong, and approve the rest. Each approval is your
            read of one card.
          </p>
        </div>
        <Link to="/decks" className={buttonVariants({ variant: 'outline' })}>
          Your decks
        </Link>
      </header>
      <p
        aria-live="polite"
        className={said?.ok === false ? 'text-sm text-destructive empty:sr-only' : 'text-sm empty:sr-only'}
      >
        {said?.text ?? ''}
      </p>
      {cards.length === 0 ? (
        <EmptyState icon={CheckCheckIcon} title="Nothing to approve in bulk">
          Every wording-only draft has your read, or none waits for one.
        </EmptyState>
      ) : (
        <>
          <div className="surface sticky top-2 z-10 flex flex-wrap items-center justify-between gap-2 p-2">
            <p className="text-sm tabular-nums">
              {chosen.size} of {cards.length} checked
            </p>
            <div className="flex flex-wrap gap-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  setAll(
                    cards.map((card) => card.oracle_id),
                    chosen.size !== cards.length,
                  )
                }
              >
                {chosen.size === cards.length ? 'Uncheck all' : 'Check all'}
              </Button>
              <Button size="sm" disabled={sending || chosen.size === 0} onClick={approve}>
                {sending ? 'Approving…' : `Approve ${chosen.size}`}
              </Button>
            </div>
          </div>
          {groups.map(({ rule, cards: grouped }) => (
            <RuleGroup
              key={rule.name}
              rule={rule}
              cards={grouped}
              chosen={chosen}
              onToggle={toggle}
              onAll={setAll}
            />
          ))}
        </>
      )}
    </section>
  )
}
