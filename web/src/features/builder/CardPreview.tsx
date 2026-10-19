import { CircleAlertIcon, InfoIcon, ShieldCheckIcon } from 'lucide-react'
import { Fragment, useId } from 'react'
import type { ComponentProps, ReactNode } from 'react'
import { Loading } from '@/components/Loading'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Skeleton } from '@/components/ui/skeleton'
import { SupportMark, supportDecoration } from '@/features/decks/SupportMark'
import { cn } from '@/lib/utils'
import type { CardsApi } from '@/net/cards'
import type { CardDetail, CardFaceView, CardHit, Reason, Support } from '@/protocol/generated'
import { segmentsOf } from './clauses'
import { ManaCost } from './ManaCost'
import { isIssue, reasonLines } from './report'
import { useCardDetail } from './useCardDetail'

type FinalFocus = ComponentProps<typeof DialogContent>['finalFocus']

interface CardPreviewProps {
  /** The card to show; keep it set while the preview closes, so its last view fades out whole. */
  card: CardHit | null
  open: boolean
  api: CardsApi
  onClose: () => void
  /** Where focus goes when the preview closes: the control that opened it. */
  finalFocus?: FinalFocus
}

// A card has several faces when its name joins them with `//` (CardHit.name).
const hasManyFaces = (card: CardHit): boolean => card.name.includes(' // ')

// A card the server holds without face rows still has its text.
function facesOf(card: CardHit, detail: CardDetail): CardFaceView[] {
  if (detail.faces.length > 0) return detail.faces
  return [
    {
      name: card.name,
      mana_cost: card.mana_cost,
      type_line: card.type_line,
      oracle_text: detail.oracle_text,
      power: null,
      toughness: null,
      loyalty: null,
      defense: null,
      colors: card.colors,
    },
  ]
}

// A stretch without the white space at its ends, which an underline should not run under.
function edges(text: string): { lead: string; core: string; tail: string } {
  const found = /^(\s*)([\s\S]*?)(\s*)$/.exec(text)
  return { lead: found?.[1] ?? '', core: found?.[2] ?? text, tail: found?.[3] ?? '' }
}

/**
 * The oracle text of one face with the words the game cannot resolve underlined with a dotted line and followed by the
 * number of their reason, which the list under the faces explains.
 */
function OracleText({
  text,
  reasons,
  face,
  support,
}: {
  text: string
  reasons: readonly Reason[]
  face: number
  support: Support
}) {
  const segments = segmentsOf(text, reasons, face)
  if (segments.length === 0)
    return <p className="text-sm text-muted-foreground">This face has no rules text.</p>
  return (
    <p data-slot="oracle-text" className="text-base whitespace-pre-line">
      {segments.map((segment, at) => {
        if (segment.covers.length === 0) return <Fragment key={at}>{segment.text}</Fragment>
        const { lead, core, tail } = edges(segment.text)
        return (
          <Fragment key={at}>
            {lead}
            {core === '' ? null : (
              <span
                data-clause={segment.covers.map((index) => index + 1).join(' ')}
                className={cn(
                  'underline decoration-dotted decoration-2 underline-offset-4',
                  supportDecoration(support),
                )}
              >
                {core}
              </span>
            )}
            {segment.ends.map((index) => (
              <sup key={index} data-note={index + 1} className="font-semibold tabular-nums">
                <span className="sr-only">Note </span>
                {index + 1}
              </sup>
            ))}
            {tail}
          </Fragment>
        )
      })}
    </p>
  )
}

// Power and toughness, loyalty and defense, for the ones the face has.
function statsOf(face: CardFaceView): ReactNode[] {
  const parts: ReactNode[] = []
  if (face.power !== null && face.toughness !== null) {
    parts.push(
      <span key="power">
        <span aria-hidden="true">
          {face.power}/{face.toughness}
        </span>
        <span className="sr-only">
          Power {face.power}, toughness {face.toughness}
        </span>
      </span>,
    )
  }
  if (face.loyalty !== null) parts.push(<span key="loyalty">Loyalty {face.loyalty}</span>)
  if (face.defense !== null) parts.push(<span key="defense">Defense {face.defense}</span>)
  return parts
}

function FaceBlock({
  face,
  index,
  many,
  reasons,
  support,
}: {
  face: CardFaceView
  index: number
  many: boolean
  reasons: readonly Reason[]
  support: Support
}) {
  const stats = statsOf(face)
  return (
    <section data-face={index} className="grid gap-2">
      {many ? (
        <h3 className="flex items-center justify-between gap-3 text-base font-semibold">
          <span className="min-w-0 break-words">{face.name}</span>
          <ManaCost cost={face.mana_cost} />
        </h3>
      ) : null}
      {many || stats.length > 0 ? (
        <p className="flex flex-wrap items-baseline justify-between gap-x-3 text-sm text-muted-foreground">
          {many ? <span>{face.type_line}</span> : null}
          {stats.length > 0 ? (
            <span className="flex gap-3 font-medium text-foreground tabular-nums">{stats}</span>
          ) : null}
        </p>
      ) : null}
      <OracleText text={face.oracle_text} reasons={reasons} face={index} support={support} />
    </section>
  )
}

/** What the game leaves to the table, one numbered line per clause, or that the card is fully supported. */
function Clauses({ support, reasons }: { support: Support; reasons: readonly Reason[] }) {
  const id = useId()
  const lines = reasonLines(reasons)
  if (lines.length > 0) {
    return (
      <section aria-labelledby={id} data-slot="clauses" className="grid gap-2 border-t pt-3">
        <h3 id={id} className="text-sm font-semibold">
          Left to the table
        </h3>
        <ol role="list" className="grid gap-2 text-sm">
          {lines.map((line, at) => (
            <li key={line.key} className="flex items-start gap-2">
              <span className="flex size-5 shrink-0 items-center justify-center rounded-full border border-border-strong text-xs font-semibold tabular-nums">
                {at + 1}
              </span>
              <span className="min-w-0 pt-px">{line.text}</span>
            </li>
          ))}
        </ol>
      </section>
    )
  }
  if (isIssue(support)) {
    return (
      <p data-slot="clauses" className="border-t pt-3 text-sm text-muted-foreground">
        The check does not say which words the game cannot resolve.
      </p>
    )
  }
  return (
    <p data-slot="clauses" className="flex items-center gap-2 border-t pt-3 text-sm">
      <ShieldCheckIcon aria-hidden="true" className="size-4 shrink-0 text-support-verified" />
      Fully supported: the game plays every word of this card by itself.
    </p>
  )
}

function Body({ card, api }: { card: CardHit; api: CardsApi }) {
  const { state, retry } = useCardDetail(api, card.oracle_id)
  const support = state.status === 'ready' ? state.detail.card.support : card.support
  return (
    <>
      <header className="grid gap-2">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <DialogTitle className="min-w-0 break-words">{card.name}</DialogTitle>
          {hasManyFaces(card) ? null : <ManaCost cost={card.mana_cost} />}
        </div>
        <DialogDescription>{card.type_line}</DialogDescription>
        <SupportMark support={support} className="justify-self-start" />
      </header>
      {state.status === 'loading' ? (
        <Loading label="Loading the card…" className="grid gap-2">
          <Skeleton className="h-4 w-1/3" />
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-5/6" />
          <Skeleton className="h-4 w-2/3" />
        </Loading>
      ) : null}
      {state.status === 'unavailable' ? (
        <Alert>
          <InfoIcon aria-hidden="true" />
          <AlertDescription className="text-foreground">
            This server has no card database, so card details are not available.
          </AlertDescription>
        </Alert>
      ) : null}
      {state.status === 'failed' ? (
        <Alert variant="destructive">
          <CircleAlertIcon aria-hidden="true" />
          <AlertDescription className="text-foreground">{state.message}</AlertDescription>
          <div className="mt-2">
            <Button type="button" variant="outline" size="sm" onClick={retry}>
              Try again
            </Button>
          </div>
        </Alert>
      ) : null}
      {state.status === 'ready' ? (
        <div className="grid gap-4">
          {facesOf(card, state.detail).map((face, index, faces) => (
            <FaceBlock
              key={`${index}:${face.name}`}
              face={face}
              index={index}
              many={faces.length > 1}
              reasons={state.detail.reasons}
              support={support}
            />
          ))}
          <Clauses support={support} reasons={state.detail.reasons} />
        </div>
      ) : null}
    </>
  )
}

/**
 * A card up close, in a dialog from the `sm` breakpoint and a sheet from the bottom under it: each face with its name,
 * cost, type line, power and toughness and oracle text, the card's support mark, and the clauses the game cannot
 * resolve underlined in the text with their reasons in words under it. The detail loads when the preview opens and
 * shows its own loading, failed and unavailable states; Escape closes it and focus returns to `finalFocus`.
 */
export function CardPreview({ card, open, api, onClose, finalFocus }: CardPreviewProps) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      {card === null ? null : (
        <DialogContent
          finalFocus={finalFocus}
          data-slot="card-preview"
          className="p-4 max-sm:top-auto max-sm:bottom-0 max-sm:left-0 max-sm:max-h-[90dvh] max-sm:w-full max-sm:max-w-none max-sm:translate-x-0 max-sm:translate-y-0 max-sm:rounded-b-none max-sm:border-x-0 max-sm:border-b-0 sm:max-w-xl sm:p-6"
        >
          <Body key={card.oracle_id} card={card} api={api} />
        </DialogContent>
      )}
    </Dialog>
  )
}
