import { useMemo } from 'react'
import {
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

/**
 * The deck in numbers, from the shared preview pieces: the copies by support state, land and spell counts, the colour
 * identity with the spells of each colour, the mana curve and the card types. The numbers follow the draft; the support
 * counts follow the check.
 */
export function StatsPanel({ deck, counts, asSaved }: StatsPanelProps) {
  const stats = useMemo(() => {
    const played = [...deck.commanders, ...deck.cards]
    const lands = landCopies(played)
    return {
      curve: manaCurve(deck.cards),
      types: typeCounts(deck.cards),
      colours: colourCounts(played),
      lands,
      spells: copiesOf(played) - lands,
    }
  }, [deck.commanders, deck.cards])
  return (
    <section aria-label="Deck statistics" data-slot="deck-stats-panel" className="@container grid gap-4">
      <div className="grid gap-2" data-slot="support-summary">
        <SupportBar counts={counts} />
        {asSaved ? (
          <p className="text-xs text-muted-foreground">As saved; the changes are being checked.</p>
        ) : null}
      </div>
      <div className="grid gap-4 @md:grid-cols-2">
        <ManaCurve curve={stats.curve} />
        <TypeBars types={stats.types} />
      </div>
      <div className="flex flex-wrap items-start gap-x-6 gap-y-3">
        <div className="grid gap-1">
          <p className="text-xs font-medium">Color identity</p>
          <IdentityPips identity={identityOfDeck(deck)} />
        </div>
        {stats.colours.length === 0 ? null : <ColourCounts colours={stats.colours} />}
        <p data-slot="land-count" className="grid gap-1 text-sm">
          <span className="text-xs font-medium">Lands and spells</span>
          <span className="tabular-nums">
            {stats.lands} {stats.lands === 1 ? 'land' : 'lands'}, {stats.spells}{' '}
            {stats.spells === 1 ? 'spell' : 'spells'}
          </span>
        </p>
      </div>
    </section>
  )
}
