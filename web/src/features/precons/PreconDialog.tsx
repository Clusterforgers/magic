import { CircleAlertIcon, PlayIcon } from 'lucide-react'
import { useState } from 'react'
import { CopyButton } from '@/components/CopyButton'
import { Loading } from '@/components/Loading'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { CardPicture, HoverPicture } from '@/features/cards/CardPicture'
import { legalityLines } from '@/features/builder/report'
import { LegalityWarning } from '@/features/decks/LegalityWarning'
import { ClauseList } from '@/features/decks/unvalidated/ClauseList'
import { clauseLines } from '@/features/decks/unvalidated/model'
import { SupportMark, supportEdge } from '@/features/decks/SupportMark'
import { ToRead } from '@/features/decks/ToRead'
import { BY_HAND, BY_HAND_TITLE, runsWhole } from '@/lib/byHand'
import { cn } from '@/lib/utils'
import type { PreconBoard, PreconListItem, PreconView } from '@/protocol/generated'
import { PreconBadge } from './PreconBadge'
import {
  BOARD_LABEL,
  checkLine,
  commandersText,
  copyList,
  flaggedLine,
  notVerified,
  releaseText,
} from './format'
import type { DetailState } from './usePreconDetail'

interface Props {
  /** The precon the dialog is about; it stays until the dialog has finished closing. */
  precon: PreconListItem
  /** False while the dialog closes. */
  open: boolean
  /** The cards and issues, as far as they have arrived. */
  detail: DetailState
  onRetry(): void
  /** Close, Escape and the close button. */
  onCancel(): void
  /** The dialog has finished closing. */
  onClosed(): void
  /** The element that takes focus when the dialog closes; null leaves that to the dialog. */
  returnFocus(): HTMLElement | null
  /** Starts a game with the precon. */
  onPlay(): void
  /** This precon's game is being made. */
  playing: boolean
  /** Some game is being made. */
  busy: boolean
}

const cardsWord = (count: number): string => `${count} ${count === 1 ? 'card' : 'cards'}`

function CardsSkeleton() {
  return (
    <div className="grid gap-2">
      {[0, 1, 2, 3, 4].map((row) => (
        <Skeleton key={row} className="h-9" />
      ))}
    </div>
  )
}

/** The rules of its format the deck breaks, as a warning: the issues once the cards are read, else their count. */
function Warnings({ precon, view }: { precon: PreconListItem; view: PreconView | null }) {
  return (
    <LegalityWarning
      slot="precon-issues"
      format={precon.format}
      issues={view === null ? precon.summary.issues : view.issues}
      lines={view === null ? undefined : legalityLines(view.issues)}
    />
  )
}

/** One board: its heading with the number of cards, then a line a card with its mark and the clauses left to the table. */
function Board({
  board,
  precon,
  onlyHand,
}: {
  board: PreconBoard
  precon: PreconListItem
  onlyHand: boolean
}) {
  const cards = onlyHand ? board.cards.filter((card) => !runsWhole(card.support)) : board.cards
  if (cards.length === 0) return null
  const id = `precon-board-${board.board}`
  return (
    <section aria-labelledby={id} data-board={board.board} className="grid gap-2">
      <h3 id={id} className="text-sm font-semibold">
        {BOARD_LABEL[board.board]}
        <span className="font-normal text-muted-foreground">{` · ${cardsWord(board.copies)}`}</span>
      </h3>
      <ul className="surface overflow-hidden">
        {cards.map((card, index) => {
          const flagged = flaggedLine(card, precon)
          return (
            <li
              key={`${card.name}:${card.printing_id ?? ''}:${index}`}
              data-support={card.support}
              className={cn(
                'flex items-start gap-3 border-b border-l-4 px-3 py-2 last:border-b-0',
                card.support === 'verified' ? 'border-l-transparent' : supportEdge(card.support),
              )}
            >
              {card.card === null ? null : (
                <HoverPicture
                  pin
                  oracleId={card.card.oracle_id}
                  name={card.name}
                  info={{ typeLine: card.card.type_line, count: card.count }}
                  className="shrink-0"
                >
                  <CardPicture
                    oracleId={card.card.oracle_id}
                    name={card.name}
                    size="small"
                    alt=""
                    quiet
                    className="w-12"
                  />
                </HoverPicture>
              )}
              <div className="grid min-w-0 flex-1 gap-1">
                <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
                  <p className="min-w-0 text-sm break-words">
                    <span className="tabular-nums text-muted-foreground">{`${card.count}× `}</span>
                    {card.card === null ? (
                      <span className="font-medium">{card.name}</span>
                    ) : (
                      <HoverPicture
                        oracleId={card.card.oracle_id}
                        name={card.name}
                        info={{ typeLine: card.card.type_line, count: card.count }}
                      >
                        <span className="font-medium">{card.name}</span>
                      </HoverPicture>
                    )}
                    {card.card === null ? null : (
                      <span className="text-muted-foreground">{` · ${card.card.type_line}`}</span>
                    )}
                  </p>
                  <SupportMark support={card.support} />
                </div>
                {flagged === null ? null : (
                  <div className="grid gap-1">
                    {card.reasons.length > 0 ? <p className="text-sm font-medium">{BY_HAND_TITLE}:</p> : null}
                    <ClauseList lines={clauseLines(flagged)} support={card.support} />
                  </div>
                )}
              </div>
            </li>
          )
        })}
      </ul>
    </section>
  )
}

/** The cards of a view, with the switch that narrows them to the cards played by hand and the button that copies those. */
function Cards({ view }: { view: PreconView }) {
  const [onlyHand, setOnlyHand] = useState(false)
  const hand = notVerified(view)
  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
        <div className="flex min-h-8 items-center gap-2 pointer-coarse:min-h-11">
          <Switch
            id="precon-only-hand"
            aria-labelledby="precon-only-hand-label"
            checked={onlyHand}
            disabled={hand === 0}
            onCheckedChange={setOnlyHand}
          />
          <Label id="precon-only-hand-label" htmlFor="precon-only-hand">
            Only cards {BY_HAND}
          </Label>
        </div>
        {hand === 0 ? null : (
          <CopyButton text={copyList(view)} label={`Copy list of the cards ${BY_HAND}`}>
            Copy list
          </CopyButton>
        )}
      </div>
      {hand === 0 ? (
        <p className="text-sm text-muted-foreground">The game plays every card of this deck as written.</p>
      ) : null}
      {view.boards.map((board) => (
        <Board key={board.board} board={board} precon={view.precon} onlyHand={onlyHand} />
      ))}
    </div>
  )
}

/**
 * The details of a precon (`GET /api/precons/{code}`): its deck check, what keeps a game from taking it, and its cards
 * by board, each with the mark of how far the game plays it and, when it does not play it whole, the clauses left to
 * the table. The list's own facts show at once; the cards arrive after.
 */
export function PreconDialog({
  precon,
  open,
  detail,
  onRetry,
  onCancel,
  onClosed,
  returnFocus,
  onPlay,
  playing,
  busy,
}: Props) {
  const view = detail.status === 'ready' ? detail.view : null
  // The view's issues are the ones of the server's check; the list item's flag says so before they arrive.
  const warned = view === null ? !precon.summary.legal : view.issues.length > 0
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onCancel()
      }}
      onOpenChangeComplete={(isOpen) => {
        if (!isOpen) onClosed()
      }}
    >
      <DialogContent
        className="flex max-h-[90dvh] max-w-3xl flex-col gap-0 overflow-hidden p-0"
        finalFocus={() => returnFocus() ?? true}
      >
        <div className="grid min-h-0 flex-1 content-start gap-4 overflow-y-auto p-6">
          <div className="grid gap-1">
            <DialogTitle>{precon.name}</DialogTitle>
            <DialogDescription>
              {[precon.set, releaseText(precon.release_date), precon.deck_type]
                .filter((part) => part !== null)
                .join(' · ')}
            </DialogDescription>
            {precon.commanders.length === 0 ? null : (
              <p className="text-sm break-words">{`Commander: ${commandersText(precon.commanders)}`}</p>
            )}
          </div>
          <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
            <PreconBadge precon={precon} />
            <p className="text-sm text-muted-foreground">{checkLine(precon)}</p>
            <ToRead count={precon.summary.to_read} target={{ precon: precon.code }} className="text-sm" />
          </div>
          {warned ? <Warnings precon={precon} view={view} /> : null}
          {detail.status === 'loading' ? (
            <Loading label="Loading the cards…">
              <CardsSkeleton />
            </Loading>
          ) : null}
          {detail.status === 'failed' ? (
            <Alert variant="destructive">
              <CircleAlertIcon aria-hidden="true" />
              <AlertTitle>Could not load the cards</AlertTitle>
              <AlertDescription>{detail.message}</AlertDescription>
              <div className="col-start-2 mt-2">
                <Button type="button" onClick={onRetry}>
                  Retry
                </Button>
              </div>
            </Alert>
          ) : null}
          {view === null ? null : <Cards view={view} />}
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2 border-t px-6 py-4">
          <Button type="button" variant="outline" onClick={onCancel}>
            Close
          </Button>
          <Button
            type="button"
            aria-label={`Play ${precon.name}`}
            disabled={busy}
            loading={playing}
            focusableWhenDisabled
            onClick={onPlay}
          >
            {playing ? null : <PlayIcon aria-hidden="true" />}
            Play
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}
