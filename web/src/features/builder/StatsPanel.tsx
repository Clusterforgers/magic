import { useMemo } from 'react'
import type { ReactNode } from 'react'
import {
  averageManaValue,
  colourCounts,
  copiesOf,
  identityOfDeck,
  landCopies,
  manaCurve,
  typeCounts,
} from '@/features/decks/preview/model'
import type { DeckPreviewData } from '@/features/decks/preview/model'
import { ColourCounts, ManaCurve, SupportBar, TypeBars } from '@/features/decks/preview/PreviewStats'
import type { Counts } from '@/protocol/generated'
import { IdentityPips } from './IdentityPips'

interface StatsPanelProps {
  /** The draft as the preview takes it (`previewFromDraft`). */
  deck: DeckPreviewData
  /** The check's copies by support state. */
  counts: Counts
  /** True while the counts are those of the saved deck and a check of the draft is on its way. */
  asSaved: boolean
}

/** One figure of the deck: a small label over a large number. */
function Figure({ label, children, slot }: { label: string; children: ReactNode; slot?: string }) {
  return (
    <div data-slot={slot} className="grid content-start gap-1 rounded-lg border bg-background/40 px-3 py-2">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="text-lg font-semibold tabular-nums">{children}</dd>
    </div>
  )
}

/** A block of the stats with a hairline edge, so each chart has room of its own; a chart's own caption titles it. */
function Block({ title, children, slot }: { title?: string; children: ReactNode; slot?: string }) {
  return (
    <div data-slot={slot} className="grid min-w-0 content-start gap-2 rounded-lg border p-3">
      {title === undefined ? null : <h4 className="text-xs font-medium">{title}</h4>}
      {children}
    </div>
  )
}

/**
 * The deck in numbers, from the shared preview pieces: a row of figures (lands, spells, the average mana value of the
 * spells, the colour identity), the copies by support state, the mana curve, the card types and the spells of each
 * colour. Each chart has a block of its own; two blocks share a row where the pane is wide enough. The numbers
 * follow the draft; the support counts follow the check.
 */
export function StatsPanel({ deck, counts, asSaved }: StatsPanelProps) {
  const stats = useMemo(() => {
    const played = [...deck.commanders, ...deck.cards]
    const lands = landCopies(played)
    return {
      curve: manaCurve(deck.cards),
      types: typeCounts(deck.cards),
      colours: colourCounts(played),
      average: averageManaValue(played),
      lands,
      spells: copiesOf(played) - lands,
    }
  }, [deck.commanders, deck.cards])
  return (
    <section aria-label="Deck statistics" data-slot="deck-stats-panel" className="@container grid gap-3">
      <dl className="grid grid-cols-2 gap-2 @lg:grid-cols-4" data-slot="deck-figures">
        <Figure label="Lands" slot="land-count">
          {stats.lands}
        </Figure>
        <Figure label="Spells">{stats.spells}</Figure>
        <Figure label="Average mana value" slot="average-mana-value">
          {stats.average === null ? '–' : stats.average.toFixed(2)}
        </Figure>
        <Figure label="Color identity">
          <IdentityPips identity={identityOfDeck(deck)} className="h-7" />
        </Figure>
      </dl>
      <Block title="Support" slot="support-summary">
        <SupportBar counts={counts} />
        {asSaved ? (
          <p className="text-xs text-muted-foreground">As saved; the changes are being checked.</p>
        ) : null}
      </Block>
      <div className="grid gap-3 @2xl:grid-cols-2">
        <Block>
          <ManaCurve curve={stats.curve} tall />
        </Block>
        <Block>
          <TypeBars types={stats.types} />
        </Block>
      </div>
      {stats.colours.length === 0 ? null : (
        <Block>
          <ColourCounts colours={stats.colours} />
        </Block>
      )}
    </section>
  )
}
