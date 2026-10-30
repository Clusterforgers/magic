import { BanIcon, CheckIcon, LoaderCircleIcon } from 'lucide-react'
import { useId, useMemo } from 'react'
import type { ReactNode } from 'react'
import { Skeleton } from '@/components/ui/skeleton'
import { IdentityPips } from '@/features/builder/IdentityPips'
import { CardPicture } from '@/features/cards/CardPicture'
import { cardsText, formatLabel } from '@/features/decks/format'
import { cn } from '@/lib/utils'
import { byHand, copiesOf, groupByKind, identityOfDeck, manaCurve, supportCounts, typeCounts } from './model'
import type { DeckPreviewData } from './model'
import { CardGroups } from './PreviewCards'
import { ManaCurve, SupportBar, TypeBars } from './PreviewStats'

/** How a format judges the deck: legal, not legal with the reasons, still being asked, or not known. */
export interface PreviewLegality {
  verdict: 'legal' | 'illegal' | 'checking' | 'unknown'
  /** The format the verdict is for, as people say it ("Commander"). */
  format: string
  /** Why the deck is not legal. */
  reasons?: readonly string[]
}

/** What a deck preview takes. */
export interface DeckPreviewProps {
  deck: DeckPreviewData
  /** How the game's format judges the deck; the line is left out without it. */
  legality?: PreviewLegality
  /** The link to the deck's not-validated cards, drawn with the support summary. */
  notValidated?: ReactNode
  /** The buttons under the header ("Use this deck"). */
  actions?: ReactNode
  /** Stands in for everything under the header while the cards load or when they could not be read. */
  body?: ReactNode
  className?: string
}

function Legality({ legality }: { legality: PreviewLegality }) {
  const { verdict, format, reasons = [] } = legality
  if (verdict === 'unknown') return null
  return (
    <div data-slot="legality" data-verdict={verdict} className="grid gap-1 text-sm">
      <p className="flex items-center gap-2 font-medium">
        {verdict === 'legal' ? (
          <CheckIcon aria-hidden="true" className="size-4 text-support-verified" />
        ) : verdict === 'illegal' ? (
          <BanIcon aria-hidden="true" className="size-4 text-destructive" />
        ) : (
          <LoaderCircleIcon aria-hidden="true" className="size-4 animate-spin text-muted-foreground" />
        )}
        {verdict === 'legal'
          ? `Legal in ${format}`
          : verdict === 'illegal'
            ? `Not legal in ${format}`
            : `Checking the deck for ${format}…`}
      </p>
      {reasons.length === 0 ? null : (
        <ul
          aria-label={`Why the deck is not legal in ${format}`}
          className="list-disc pl-6 text-xs text-destructive"
        >
          {[...new Set(reasons)].map((reason) => (
            <li key={reason} className="[overflow-wrap:anywhere]">
              {reason}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

/**
 * A deck at a glance, from the card lists alone: its commanders as large pictures, name and source, legality in the
 * game's format, colour identity and size, how many cards the game runs whole and how many are played by hand (with a
 * link to the list), the mana curve and the types as small bars, and every card as a picture grouped by type with
 * count badges, a mark on the ones played by hand and a large view on a click. Shared by the lobby's deck chooser and
 * the deck builder.
 */
export function DeckPreview({ deck, legality, notValidated, actions, body, className }: DeckPreviewProps) {
  const titleId = useId()
  const { commanders, cards, sideboard } = deck
  const stats = useMemo(() => {
    const played = [...commanders, ...cards]
    return {
      groups: groupByKind(cards),
      curve: manaCurve(cards),
      types: typeCounts(cards),
      support: supportCounts(played),
      size: copiesOf(played),
      sideGroups: groupByKind(sideboard),
    }
  }, [commanders, cards, sideboard])
  const identity = identityOfDeck(deck)
  const total = stats.support.total
  const hand = byHand(stats.support)
  return (
    <article
      aria-labelledby={titleId}
      data-slot="deck-preview"
      className={cn('@container grid min-w-0 content-start gap-4', className)}
    >
      <header className="flex min-w-0 flex-wrap gap-4">
        {commanders.length === 0 ? null : (
          <ul
            aria-label="Commanders"
            data-slot="commanders"
            className={cn('flex shrink-0 gap-2', commanders.length > 1 && 'max-[28rem]:flex-col')}
          >
            {commanders.map((card) => (
              <li key={card.oracleId} data-commander={card.oracleId}>
                <CardPicture
                  oracleId={card.oracleId}
                  name={card.name}
                  size="normal"
                  alt={card.name}
                  className={commanders.length > 1 ? 'w-32' : 'w-40'}
                />
              </li>
            ))}
          </ul>
        )}
        <div className="grid min-w-0 flex-1 basis-44 content-start gap-2">
          <div className="grid gap-1">
            <h3 id={titleId} className="text-lg leading-tight font-semibold [overflow-wrap:anywhere]">
              {deck.name}
            </h3>
            <p className="text-sm text-muted-foreground">
              {deck.source}, {formatLabel(deck.format)}
            </p>
            {commanders.length === 0 ? null : (
              <p className="text-xs text-muted-foreground [overflow-wrap:anywhere]">
                {commanders.map((card) => card.name).join(' and ')}
              </p>
            )}
          </div>
          {legality === undefined ? null : <Legality legality={legality} />}
          <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm">
            <IdentityPips identity={identity} />
            <span data-slot="deck-size" className="tabular-nums">
              {cardsText(stats.size)}
            </span>
          </div>
          {actions === undefined ? null : <div className="flex flex-wrap items-center gap-2">{actions}</div>}
        </div>
      </header>

      {body !== undefined ? (
        body
      ) : (
        <>
          <section aria-label="Support" data-slot="support-summary" className="grid gap-2">
            <p className="text-sm">
              {total === 0
                ? 'No cards to count.'
                : hand === 0
                  ? 'The whole deck runs.'
                  : `${hand} of ${total} cards are played by hand.`}
            </p>
            <SupportBar counts={stats.support} />
            {notValidated === undefined || hand === 0 ? null : <div className="text-sm">{notValidated}</div>}
          </section>

          {stats.size === 0 ? null : (
            <div className="grid gap-4 @md:grid-cols-2" data-slot="deck-stats">
              <ManaCurve curve={stats.curve} />
              <TypeBars types={stats.types} />
            </div>
          )}

          <CardGroups groups={stats.groups} />
          {stats.sideGroups.length === 0 ? null : (
            <section aria-label="Sideboard" data-slot="sideboard" className="grid gap-3">
              <h4 className="border-b pb-1 text-sm font-semibold">Sideboard</h4>
              <CardGroups groups={stats.sideGroups} />
            </section>
          )}
        </>
      )}
    </article>
  )
}

/** What stands under the header while the cards load: the support line, the bars and a grid of cards. */
export function DeckPreviewSkeleton({ className }: { className?: string }) {
  return (
    <div data-slot="deck-preview-loading" aria-busy="true" className={cn('grid gap-4', className)}>
      <span className="sr-only">Loading the cards…</span>
      <Skeleton className="h-16" />
      <Skeleton className="h-24" />
      <div className="grid grid-cols-[repeat(auto-fill,minmax(4.75rem,1fr))] gap-3">
        {Array.from({ length: 12 }, (_, index) => (
          <Skeleton key={index} className="aspect-[5/7] rounded-lg" />
        ))}
      </div>
    </div>
  )
}
