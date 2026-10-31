import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { unvalidatedLink } from '@/features/decks/format'
import { SupportMark } from '@/features/decks/SupportMark'
import { BY_HAND, cardsByHand, runsWhole } from '@/lib/byHand'
import { copiesOf, supportCounts } from './model'
import type { DeckPreviewData, PreviewCard } from './model'

/** The cards of a deck the game does not run whole, commanders first. */
export const notValidatedOf = (deck: DeckPreviewData): PreviewCard[] =>
  [...deck.commanders, ...deck.cards].filter((card) => !runsWhole(card.support))

/** The words of the link to a deck's not-validated cards, as the deck list says them: "Show the 7 cards played by hand". */
export const notValidatedWords = (deck: DeckPreviewData): string =>
  unvalidatedLink(supportCounts([...deck.commanders, ...deck.cards]))

/** A button that lists the cards of a deck that are played by hand, each with its mark and the reasons. */
export function NotValidatedDialog({ deck }: { deck: DeckPreviewData }) {
  const [open, setOpen] = useState(false)
  const cards = notValidatedOf(deck)
  return (
    <>
      <Button
        type="button"
        variant="link"
        size="xs"
        aria-haspopup="dialog"
        className="h-auto px-0"
        onClick={() => setOpen(true)}
      >
        {notValidatedWords(deck)}
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent data-slot="not-validated">
          <DialogTitle>Not validated cards of {deck.name}</DialogTitle>
          <DialogDescription>
            {cardsByHand(copiesOf(cards))}: the game does not run {copiesOf(cards) === 1 ? 'it' : 'them'}{' '}
            whole.
          </DialogDescription>
          <ul aria-label={`Cards ${BY_HAND}`} className="grid gap-3">
            {cards.map((card) => (
              <li key={card.oracleId} className="grid gap-1 border-b pb-2 last:border-b-0">
                <span className="flex flex-wrap items-center gap-2 text-sm font-medium">
                  {card.name}
                  <SupportMark support={card.support} />
                </span>
                {[...new Set(card.reasons)].map((reason) => (
                  <span key={reason} className="text-xs text-muted-foreground [overflow-wrap:anywhere]">
                    {reason}
                  </span>
                ))}
              </li>
            ))}
          </ul>
        </DialogContent>
      </Dialog>
    </>
  )
}
