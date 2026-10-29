import { HandIcon, ShieldHalfIcon } from 'lucide-react'
import { useState } from 'react'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { CardPicture, HoverPicture } from '@/features/cards/CardPicture'
import { SupportMark } from '@/features/decks/SupportMark'
import { cn } from '@/lib/utils'
import type { Support } from '@/protocol/generated'
import { copiesOf } from './model'
import type { CardGroup, PreviewCard } from './model'

const PLAYED_BY_HAND: Readonly<Record<Support, string>> = {
  verified: 'runs whole',
  partial: 'partly played by hand',
  manual: 'played by hand',
  stale: 'played by hand until it is checked again',
  unplayable: 'the game cannot play it',
}

/** The words that name a card with its copies and how far the game plays it: a tile's accessible name. */
export function cardLabel(card: PreviewCard): string {
  const copies = card.count > 1 ? `${card.count} copies of ` : ''
  return `${copies}${card.name}${card.support === 'verified' ? '' : `, ${PLAYED_BY_HAND[card.support]}`}`
}

/** The large picture of one card with its type, its support and why it is not Verified. */
export function CardZoom({ card, onClose }: { card: PreviewCard | null; onClose: () => void }) {
  return (
    <Dialog open={card !== null} onOpenChange={(open) => (open ? undefined : onClose())}>
      <DialogContent className="max-w-sm justify-items-center" data-slot="card-zoom">
        {card === null ? null : (
          <>
            <CardPicture oracleId={card.oracleId} name={card.name} size="large" alt="" className="w-64" />
            <div className="grid w-full justify-items-center gap-2 text-center">
              <DialogTitle className="text-base">
                {card.name}
                {card.count > 1 ? (
                  <span className="font-normal text-muted-foreground"> ×{card.count}</span>
                ) : null}
              </DialogTitle>
              <DialogDescription>{card.typeLine}</DialogDescription>
              <SupportMark support={card.support} />
              {card.reasons.length === 0 ? null : (
                <ul
                  aria-label="Played by hand"
                  className="grid gap-1 text-left text-xs text-muted-foreground"
                >
                  {card.reasons.map((reason) => (
                    <li key={reason} className="[overflow-wrap:anywhere]">
                      {reason}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}

function Tile({ card, onOpen }: { card: PreviewCard; onOpen: (card: PreviewCard) => void }) {
  const hand = card.support !== 'verified'
  return (
    <li data-card={card.oracleId} data-support={card.support} className="min-w-0">
      <HoverPicture oracleId={card.oracleId} name={card.name} className="block">
        <button
          type="button"
          aria-haspopup="dialog"
          aria-label={cardLabel(card)}
          onClick={() => onOpen(card)}
          className={cn(
            'relative block w-full rounded-[4.75%/3.393%] outline-offset-2 transition-transform duration-fast ease-out hover:-translate-y-0.5',
            hand && 'ring-2 ring-support-manual/70',
          )}
        >
          <CardPicture oracleId={card.oracleId} name={card.name} size="small" alt="" className="w-full" />
          {card.count > 1 ? (
            <span
              data-slot="count-badge"
              aria-hidden="true"
              className="absolute -top-1.5 -right-1.5 grid min-w-6 place-items-center rounded-full border bg-popover px-1 text-xs font-semibold tabular-nums shadow-e1"
            >
              ×{card.count}
            </span>
          ) : null}
          {hand ? (
            <span
              data-slot="support-badge"
              aria-hidden="true"
              className="absolute -bottom-1.5 -left-1.5 grid size-6 place-items-center rounded-full border border-support-manual bg-popover text-support-manual shadow-e1"
            >
              {card.support === 'partial' ? (
                <ShieldHalfIcon className="size-3.5" />
              ) : (
                <HandIcon className="size-3.5" />
              )}
            </span>
          ) : null}
        </button>
      </HoverPicture>
    </li>
  )
}

/** The cards of a deck grouped by type as small pictures with count badges and a mark on the ones played by hand; each opens large. */
export function CardGroups({ groups }: { groups: readonly CardGroup[] }) {
  const [zoomed, setZoomed] = useState<PreviewCard | null>(null)
  return (
    <div data-slot="card-groups" className="grid gap-4">
      {groups.map((group) => (
        <section key={group.kind} aria-label={group.label} data-group={group.kind} className="grid gap-2">
          <h4 className="flex items-baseline justify-between gap-2 border-b pb-1 text-sm font-semibold">
            {group.label}
            <span className="font-normal text-muted-foreground tabular-nums">{copiesOf(group.cards)}</span>
          </h4>
          <ul className="grid grid-cols-[repeat(auto-fill,minmax(4.75rem,1fr))] gap-3 pt-2">
            {group.cards.map((card) => (
              <Tile key={card.oracleId} card={card} onOpen={setZoomed} />
            ))}
          </ul>
        </section>
      ))}
      <CardZoom card={zoomed} onClose={() => setZoomed(null)} />
    </div>
  )
}
