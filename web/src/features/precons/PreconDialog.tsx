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
import { legalityLines, sentence } from '@/features/builder/report'
import { SupportMark, supportEdge } from '@/features/decks/SupportMark'
import { cn } from '@/lib/utils'
import type { PreconBoard, PreconListItem, PreconView } from '@/protocol/generated'
import { PreconBadge } from './PreconBadge'
import {
  BOARD_LABEL,
  checkLine,
  clausesText,
  commandersText,
  copyList,
  notVerified,
  releaseText,
  statusOf,
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

/** Why a game will not take the deck: the server's issues, or the cards it cannot play. */
function Blocked({ view }: { view: PreconView | null }) {
  const lines = view === null ? [] : legalityLines(view.issues)
  return (
    <Alert variant="destructive" data-slot="precon-issues">
      <CircleAlertIcon aria-hidden="true" />
      <AlertTitle>Cannot be taken</AlertTitle>
      <AlertDescription className="text-foreground">
        {lines.length > 0 ? (
          <ul className="grid list-disc gap-1 pl-5">
            {lines.map((line) => (
              <li key={line.key}>{line.text}</li>
            ))}
          </ul>
        ) : view === null ? (
          'The game will not take this deck.'
        ) : (
          'The deck holds cards the game cannot play. They are marked Unplayable below.'
        )}
      </AlertDescription>
    </Alert>
  )
}

/** One board: its heading with the number of cards, then a line a card with its mark and the clauses left to the table. */
function Board({ board, onlyHand }: { board: PreconBoard; onlyHand: boolean }) {
  const cards = onlyHand ? board.cards.filter((card) => card.support !== 'verified') : board.cards
  if (cards.length === 0) return null
  const id = `precon-board-${board.board}`
  return (
    <section aria-labelledby={id} data-board={board.board} className="grid gap-2">
      <h3 id={id} className="text-sm font-semibold">
        {BOARD_LABEL[board.board]}
        <span className="font-normal text-muted-foreground">{` · ${cardsWord(board.copies)}`}</span>
      </h3>
      <ul className="surface overflow-hidden">
        {cards.map((card, index) => (
          <li
            key={`${card.name}:${card.printing_id ?? ''}:${index}`}
            data-support={card.support}
            className={cn(
              'grid gap-1 border-b border-l-4 px-3 py-2 last:border-b-0',
              card.support === 'verified' ? 'border-l-transparent' : supportEdge(card.support),
            )}
          >
            <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
              <p className="min-w-0 text-sm break-words">
                <span className="tabular-nums text-muted-foreground">{`${card.count}× `}</span>
                <span className="font-medium">{card.name}</span>
                {card.card === null ? null : (
                  <span className="text-muted-foreground">{` · ${card.card.type_line}`}</span>
                )}
              </p>
              <SupportMark support={card.support} />
            </div>
            {card.support === 'verified' ? null : (
              <p className="text-sm break-words text-muted-foreground">
                {card.reasons.length > 0 ? (
                  <span className="font-medium text-foreground">Played by hand: </span>
                ) : null}
                {sentence(clausesText(card.support, card.reasons))}
              </p>
            )}
          </li>
        ))}
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
            Only cards played by hand
          </Label>
        </div>
        {hand === 0 ? null : (
          <CopyButton text={copyList(view)} label="Copy list of the cards played by hand">
            Copy list
          </CopyButton>
        )}
      </div>
      {hand === 0 ? (
        <p className="text-sm text-muted-foreground">The game plays every card of this deck as written.</p>
      ) : null}
      {view.boards.map((board) => (
        <Board key={board.board} board={board} onlyHand={onlyHand} />
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
  const blocked = statusOf(precon) === 'blocked'
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
          </div>
          {blocked && detail.status !== 'loading' ? <Blocked view={view} /> : null}
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
            disabled={blocked || busy}
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
