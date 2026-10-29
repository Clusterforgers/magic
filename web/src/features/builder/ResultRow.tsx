import { HoverPicture } from '@/features/cards/CardPicture'
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
import { SupportGlyph } from '@/features/decks/SupportMark'
import { FixMarkButton } from '@/features/fixmarks/FixMarkButton'
import type { FixMarksApi } from '@/net/fixMarks'
import type { CardHit, DeckZone } from '@/protocol/generated'
import { copiesOf, hasRoom } from './deckEdit'
import { outsideOf, outsideWords } from './identity'
import type { Draft } from './deckEdit'
import { ManaCost } from './ManaCost'
import { ZONE_TITLE, zonesFor } from './zones'

interface ResultRowProps {
  card: CardHit
  draft: Draft
  /** The identity of the deck's commanders, or null while it has none; a card with colours outside it says so. */
  identity?: readonly string[] | null
  onAdd: (card: CardHit, zone: DeckZone) => void
  /** Opens the preview of the card; `opener` is the control focus returns to. */
  onPreview: (card: CardHit, opener: HTMLElement) => void
  /** The fix-mark endpoints; with them a card that is not Verified gets the flag. */
  fixMarks?: FixMarksApi
}

/**
 * One search hit: the row is the button that adds a copy to the main deck (Enter and Space do too), the name on top of
 * it opens the card's preview, and a menu beside it adds to the sideboard, the command zone or the companion slot. It
 * says how many copies the deck holds.
 */
export function ResultRow({ card, draft, identity = null, onAdd, onPreview, fixMarks }: ResultRowProps) {
  const outside = identity === null ? [] : outsideOf(card, identity)
  const describedBy = useId()
  const inDeck = copiesOf(draft, card.oracle_id)
  const full = !hasRoom(draft)
  const others = zonesFor(draft.format).filter((zone) => zone !== 'main')
  const issue = card.support !== 'verified'
  return (
    <li data-card={card.oracle_id} className="flex items-center gap-1">
      {/* The add button covers the body without holding it, since the name inside is a button of its own. The one
          column may shrink below its longest line (`minmax(0, 1fr)`), so a long name is cut with an ellipsis instead
          of pushing the menu button out of the row. */}
      <div
        data-slot="result-body"
        data-full={full || undefined}
        className="relative grid min-h-11 min-w-0 flex-1 grid-cols-[minmax(0,1fr)] rounded-lg px-2 py-1 transition-colors duration-fast ease-out hover:bg-accent data-full:opacity-50"
      >
        <span className="flex min-w-0 items-center justify-between gap-2">
          <HoverPicture oracleId={card.oracle_id} name={card.name}>
            <button
              type="button"
              aria-haspopup="dialog"
              title={card.name}
              onClick={(event) => onPreview(card, event.currentTarget)}
              className="relative z-10 -my-1 min-w-0 truncate rounded-sm py-1 text-left text-sm font-medium underline-offset-4 hover:underline"
            >
              {card.name}
            </button>
          </HoverPicture>
          <span className="flex shrink-0 items-center gap-2">
            <ManaCost cost={card.mana_cost} />
            {issue ? <SupportGlyph support={card.support} className="relative z-10" /> : null}
          </span>
        </span>
        <span
          id={describedBy}
          className="flex min-w-0 items-center justify-between gap-2 text-xs text-muted-foreground"
        >
          <span className="truncate">{card.type_line}</span>
          {outside.length > 0 ? (
            <span data-slot="outside-identity" className="shrink-0 font-medium text-destructive">
              {outsideWords(outside)}
            </span>
          ) : null}
          {inDeck > 0 ? <span className="shrink-0 font-medium text-foreground">{inDeck} in deck</span> : null}
        </span>
        <button
          type="button"
          aria-label={`Add ${card.name} to main deck`}
          aria-describedby={describedBy}
          aria-disabled={full || undefined}
          onClick={() => {
            if (!full) onAdd(card, 'main')
          }}
          className="absolute inset-0 rounded-lg aria-disabled:cursor-not-allowed"
        />
      </div>
      {issue && fixMarks !== undefined ? (
        <FixMarkButton compact oracleId={card.oracle_id} name={card.name} api={fixMarks} />
      ) : null}
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
