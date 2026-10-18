import { ColorDot } from '@/components/ColorDot'
import type { DeckInfo } from '@/net/api'

/** How many cards the one-line preview names. */
const PREVIEW_CARDS = 4

// The deck's lines, most copies first and then by name.
function byCopies(deck: DeckInfo): DeckInfo['cards'] {
  return [...deck.cards].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
}

/** A deck at a glance: its colour and size, the cards it has most of, and the whole list on request. */
export function DeckPreview({ deck }: { deck: DeckInfo }) {
  const cards = byCopies(deck)
  const hidden = cards.length - PREVIEW_CARDS
  const preview = cards
    .slice(0, PREVIEW_CARDS)
    .map((card) => `${card.name} ×${card.count}`)
    .join(', ')
  return (
    <div className="grid gap-1 text-xs text-muted-foreground">
      <p className="flex items-center gap-1.5">
        <ColorDot color={deck.color} />
        <span>
          {deck.color}, {deck.size} cards
        </span>
      </p>
      <p>
        {preview}
        {hidden > 0 ? `, and ${hidden} more` : ''}
      </p>
      <details>
        <summary className="w-fit cursor-pointer underline-offset-2 hover:underline">
          Full list of {deck.name}
        </summary>
        <ul className="mt-1 grid max-h-40 gap-x-4 overflow-y-auto sm:grid-cols-2">
          {cards.map((card) => (
            <li key={card.name}>
              {card.count} × {card.name}
            </li>
          ))}
        </ul>
      </details>
    </div>
  )
}
