import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { SupportMark } from '@/features/decks/SupportMark'
import { copiesOf } from './model'
import type { DeckPreviewData, PreviewCard } from './model'

/** The cards of a deck the game does not run whole, commanders first. */
export const notValidatedOf = (deck: DeckPreviewData): PreviewCard[] =>
  [...deck.commanders, ...deck.cards].filter((card) => card.support !== 'verified')

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
        Not validated cards
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent data-slot="not-validated">
          <DialogTitle>Not validated cards of {deck.name}</DialogTitle>
          <DialogDescription>
            {copiesOf(cards)} {copiesOf(cards) === 1 ? 'card is' : 'cards are'} played by hand: the game does
            not run {copiesOf(cards) === 1 ? 'it' : 'them'} whole.
          </DialogDescription>
          <ul aria-label="Cards played by hand" className="grid gap-3">
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
