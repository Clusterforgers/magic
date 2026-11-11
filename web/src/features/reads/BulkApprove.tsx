import { Link } from '@tanstack/react-router'
import { CheckCheckIcon } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { Button, buttonVariants } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { BulkCardRow } from '@/features/bulk-reads/BulkReadView'
import { wordingOnly } from '@/features/bulk-reads/model'
import { describeDeckError } from '@/features/decks/errors'
import { forgetToReadCount } from './useToReadCount'
import type { BulkReadsApi } from '@/net/bulkReads'
import type { BulkCard } from '@/protocol/generated'

interface BulkApproveProps {
  api: BulkReadsApi
  /** The cards to read the reader has not judged yet; only those are offered. */
  open: ReadonlySet<string>
  /** Hears of the cards the server took a `correct` for. */
  onApproved: (ids: readonly string[]) => void
  /** Hears when the dialog opens or closes, so the page's keys rest meanwhile. */
  onOpenChange?: (open: boolean) => void
}

/**
 * The review's bulk approval (docs/web.md, Reviewing cards): the open cards to read whose every differing clause
 * matches the card by wording alone, offered in one strip; its dialog shows each pair with the changed words marked,
 * every card checked, and approves the checked ones as read right. Nothing shows while none is offered.
 */
export function BulkApprove({ api, open, onApproved, onOpenChange }: BulkApproveProps) {
  const [cards, setCards] = useState<readonly BulkCard[]>([])
  const [round, setRound] = useState(0)
  const [showing, setShowing] = useState(false)
  const [chosen, setChosen] = useState<ReadonlySet<string>>(new Set())
  const [sending, setSending] = useState(false)
  const [said, setSaid] = useState<{ ok: boolean; text: string } | null>(null)

  useEffect(() => {
    let live = true
    api.list().then(
      (reads) => {
        if (live) setCards(reads.cards)
      },
      // Without the list the cards are read one by one, as before.
      () => undefined,
    )
    return () => {
      live = false
    }
  }, [api, round])

  const offered = useMemo(() => cards.filter((card) => open.has(card.oracle_id)), [cards, open])
  const show = (on: boolean): void => {
    setShowing(on)
    onOpenChange?.(on)
    if (on) {
      setChosen(new Set(offered.map((card) => card.oracle_id)))
      setSaid(null)
    }
  }
  const picked = offered.filter((card) => chosen.has(card.oracle_id)).length
  const toggle = (id: string): void => {
    const next = new Set(chosen)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    setChosen(next)
  }
  const approve = (): void => {
    const sent = offered.filter((card) => chosen.has(card.oracle_id))
    setSending(true)
    setSaid(null)
    api
      .approve({ cards: sent.map((card) => ({ oracle_id: card.oracle_id, read_hash: card.read_hash })) })
      .then(
        (result) => {
          const stale = result.stale.length
          onApproved(result.approved)
          forgetToReadCount()
          setRound((n) => n + 1)
          setSaid({
            ok: true,
            text: `${wordingOnly(result.approved.length, 'approved')}.${stale === 0 ? '' : ` ${wordingOnly(stale, 'changed since the page loaded and left alone')}.`}`,
          })
          if (stale === 0) show(false)
        },
        (error: unknown) => setSaid({ ok: false, text: describeDeckError(error) }),
      )
      .finally(() => setSending(false))
  }

  return (
    <>
      <p aria-live="polite" className="text-sm empty:sr-only" data-slot="bulk-said">
        {said?.ok === true && !showing ? said.text : ''}
      </p>
      {offered.length === 0 ? null : (
        <div
          data-slot="bulk-offer"
          className="surface flex flex-wrap items-center justify-between gap-x-4 gap-y-2 px-3 py-2"
        >
          <p className="flex min-w-0 items-center gap-2 text-sm">
            <CheckCheckIcon aria-hidden="true" className="size-4 shrink-0 text-support-read" />
            {offered.length === 1
              ? '1 card differs from its Oracle text in wording alone.'
              : `${offered.length} cards differ from their Oracle text in wording alone.`}
          </p>
          <Button size="sm" variant="outline" onClick={() => show(true)}>
            {`Approve ${offered.length} wording-only`}
          </Button>
        </div>
      )}
      <Dialog open={showing} onOpenChange={show}>
        <DialogContent data-slot="bulk-dialog" className="max-w-5xl">
          <DialogTitle>Approve wording-only reads</DialogTitle>
          <DialogDescription>
            The game words these cards differently, and a fixed rule says the two mean the same. Glance over
            each pair, uncheck any that reads wrong, and approve the rest; each approval is your read of one
            card.
          </DialogDescription>
          <div
            aria-hidden="true"
            className="hidden px-3 text-xs text-muted-foreground md:grid md:grid-cols-[1.25rem_12rem_minmax(0,1fr)_minmax(0,1fr)] md:gap-4"
          >
            <span />
            <span>Card name</span>
            <span>Oracle text</span>
            <span>The game says</span>
          </div>
          <ul className="surface">
            {offered.map((card) => (
              <BulkCardRow
                key={card.oracle_id}
                card={card}
                checked={chosen.has(card.oracle_id)}
                onToggle={() => toggle(card.oracle_id)}
              />
            ))}
          </ul>
          {said === null ? null : (
            <p
              role={said.ok ? 'status' : 'alert'}
              className={said.ok ? 'text-sm' : 'text-sm text-destructive'}
            >
              {said.text}
            </p>
          )}
          <div className="sticky -bottom-6 -mx-6 -mb-6 flex flex-wrap items-center justify-between gap-2 border-t bg-popover px-6 py-4">
            <Link to="/read/bulk" className={buttonVariants({ variant: 'link', size: 'sm' })}>
              Every wording-only card, by rule
            </Link>
            <div className="flex gap-2">
              <Button variant="ghost" size="sm" onClick={() => show(false)}>
                Cancel
              </Button>
              <Button size="sm" disabled={sending || picked === 0} onClick={approve}>
                {sending ? 'Approving…' : `Approve ${picked}`}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}
