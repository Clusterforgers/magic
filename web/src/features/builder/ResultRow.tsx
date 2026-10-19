import { EllipsisVerticalIcon } from 'lucide-react'
import { useId } from 'react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import type { CardHit, DeckZone } from '@/protocol/generated'
import { copiesOf, hasRoom } from './deckEdit'
import type { Draft } from './deckEdit'
import { ManaCost } from './ManaCost'
import { ZONE_TITLE, zonesFor } from './zones'

interface ResultRowProps {
  card: CardHit
  draft: Draft
  onAdd: (card: CardHit, zone: DeckZone) => void
}

/**
 * One search hit: the whole row is the button that adds a copy to the main deck (Enter and Space do too), and a menu
 * beside it adds to the sideboard, the command zone or the companion slot. It says how many copies the deck holds.
 */
export function ResultRow({ card, draft, onAdd }: ResultRowProps) {
  const describedBy = useId()
  const inDeck = copiesOf(draft, card.oracle_id)
  const full = !hasRoom(draft)
  const others = zonesFor(draft.format).filter((zone) => zone !== 'main')
  return (
    <li data-card={card.oracle_id} className="flex items-center gap-1">
      <button
        type="button"
        aria-label={`Add ${card.name} to main deck`}
        aria-describedby={describedBy}
        aria-disabled={full || undefined}
        onClick={() => {
          if (!full) onAdd(card, 'main')
        }}
        className="grid min-h-11 min-w-0 flex-1 rounded-lg px-2 py-1 text-left transition-colors duration-fast ease-out hover:bg-accent aria-disabled:cursor-not-allowed aria-disabled:opacity-50"
      >
        <span className="flex min-w-0 items-center justify-between gap-2">
          <span className="truncate text-sm font-medium">{card.name}</span>
          <ManaCost cost={card.mana_cost} />
        </span>
        <span
          id={describedBy}
          className="flex min-w-0 items-center justify-between gap-2 text-xs text-muted-foreground"
        >
          <span className="truncate">{card.type_line}</span>
          {inDeck > 0 ? <span className="shrink-0 font-medium text-foreground">{inDeck} in deck</span> : null}
        </span>
      </button>
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button variant="ghost" size="icon" aria-label={`Add ${card.name} to another part of the deck`} />
          }
        >
          <EllipsisVerticalIcon aria-hidden="true" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuGroup>
            <DropdownMenuLabel>Add to</DropdownMenuLabel>
            {others.map((zone) => (
              <DropdownMenuItem key={zone} onClick={() => !full && onAdd(card, zone)}>
                {ZONE_TITLE[zone]}
              </DropdownMenuItem>
            ))}
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>
    </li>
  )
}
