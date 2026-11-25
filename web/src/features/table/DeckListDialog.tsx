/**
 * A seat's deck as the viewer may look through it (`DeckPictures.decks`, `deckLists.ts`): the viewer's own and every
 * public list (an unchanged precon, the open-deck house rule) whole by group with their pictures, of a hidden deck only
 * its command zone, and the tokens the listed cards make. Opened from the seat's library chip and the table menu.
 */
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { CardImage } from '@/features/cards/CardImage'
import { CardPicture, HoverPicture } from '@/features/cards/CardPicture'
import { cardImageUrl, missKeyOf } from '@/features/cards/images'
import { useImageChoice } from '@/features/cards/imageSetting'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ListCard, PlayerId, SeatList, TokenPicture } from '@/protocol/generated'
import { hasMore, listOf, listSections, listWords, tokenLabel, tokenSource } from './deckLists'
import { useSeatNamer, useViewerSeat } from './names'
import { DENSITY, ZONE_ICON } from './PodParts'
import type { ChipDensity } from './PodParts'
import { zoneLabel } from './podFigures'

/** A group's cells: pictures 76 px wide side by side, wrapping within the group. */
const ROW = 'flex flex-wrap gap-2 *:w-[76px]'

/** The list of `seat` as the server last told it, or null. */
function useSeatList(seat: PlayerId): SeatList | null {
  return usePlay((state) => listOf(state.deckLists, seat))
}

/** One line: the picture (large on hover, pinned by a press) with its copies, or its name with Text only. */
function Line({ card }: { card: ListCard }) {
  const images = useImageChoice()
  const copies = card.count > 1 ? `×${card.count}` : null
  const words = card.count > 1 ? `${card.count} ${card.name}` : card.name
  if (images === 'text' || card.identity === undefined) {
    return (
      <li data-slot="deck-line" className="flex items-baseline gap-1 text-sm">
        {copies === null ? null : <span className="tabular-nums text-muted-foreground">{card.count}</span>}
        <span>{card.name}</span>
      </li>
    )
  }
  const printingId = card.identity.printing_id ?? null
  return (
    <li data-slot="deck-line" aria-label={words} className="relative">
      <HoverPicture
        oracleId={card.identity.oracle_id}
        printingId={printingId}
        name={card.name}
        pin
        className="block"
      >
        <CardPicture
          oracleId={card.identity.oracle_id}
          printingId={printingId}
          name={card.name}
          size="small"
          alt={card.name}
          pulse
          className="w-full"
        />
      </HoverPicture>
      {copies === null ? null : (
        <span
          aria-hidden="true"
          className="pointer-events-none absolute right-1 bottom-1 rounded-md bg-background/90 px-1 text-xs font-semibold tabular-nums shadow-e1"
        >
          {copies}
        </span>
      )}
    </li>
  )
}

/** A token the deck can make: its picture over a tile that names it, and its name and size under it. */
function Token({ token }: { token: TokenPicture }) {
  const images = useImageChoice()
  const label = tokenLabel(token)
  if (images === 'text') {
    return (
      <li data-slot="deck-token" className="text-sm">
        {label}
      </li>
    )
  }
  const source = tokenSource(token)
  return (
    <li data-slot="deck-token" className="grid gap-1">
      <CardImage
        src={cardImageUrl(source, 0, 'small')}
        missKey={missKeyOf(source, 0, 'small')}
        alt={label}
        className="aspect-[5/7] w-full"
      >
        <span
          aria-hidden="true"
          className="grid size-full place-items-center rounded-[4.75%/3.393%] border bg-muted p-1 text-center text-xs text-muted-foreground"
        >
          {label}
        </span>
      </CardImage>
      <span
        aria-hidden="true"
        data-slot="deck-token-name"
        className="truncate text-center text-xs text-muted-foreground"
      >
        {label}
      </span>
    </li>
  )
}

/** The body of a list: its groups, then its tokens. */
function ListBody({ list, owner }: { list: SeatList; owner: string }) {
  const images = useImageChoice()
  const grid = images === 'text' ? 'grid gap-0.5' : ROW
  return (
    <div data-slot="deck-list" data-whole={list.whole} className="flex flex-wrap gap-x-6 gap-y-4">
      {listSections(list.cards).map((section) => (
        <section
          key={section.group}
          aria-label={`${section.heading} (${section.copies})`}
          className="grid min-w-0 content-start gap-2"
        >
          <h3 className="text-sm font-medium">
            {section.heading} <span className="text-muted-foreground tabular-nums">{section.copies}</span>
          </h3>
          <ul className={grid}>
            {section.cards.map((card) => (
              <Line
                key={`${card.name}:${card.identity?.printing_id ?? card.identity?.oracle_id ?? ''}`}
                card={card}
              />
            ))}
          </ul>
        </section>
      ))}
      {list.tokens.length === 0 ? null : (
        <section
          data-slot="deck-tokens"
          aria-label={`Tokens ${owner} can make`}
          className="grid min-w-0 basis-full content-start gap-2"
        >
          <h3 className="text-sm font-medium">
            Tokens <span className="text-muted-foreground tabular-nums">{list.tokens.length}</span>
          </h3>
          <ul className={grid}>
            {list.tokens.map((token, at) => (
              <Token key={at} token={token} />
            ))}
          </ul>
        </section>
      )}
    </div>
  )
}

interface DeckListDialogProps {
  seat: PlayerId
  open: boolean
  onOpenChange: (open: boolean) => void
}

/** The dialog of a seat's list; nothing while the server told none for it. */
export function DeckListDialog({ seat, open, onOpenChange }: DeckListDialogProps) {
  const list = useSeatList(seat)
  const name = useSeatNamer()(seat)
  const own = useViewerSeat() === seat
  if (list === null) return null
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-slot="deck-list-dialog">
        <DialogTitle>{own ? 'Your deck' : `${name}’s deck`}</DialogTitle>
        <DialogDescription>{listWords(list, name, own)}</DialogDescription>
        <ListBody list={list} owner={own ? 'you' : name} />
      </DialogContent>
    </Dialog>
  )
}

interface LibraryChipProps {
  seat: PlayerId
  owner: string
  count: number
  density: ChipDensity
}

/**
 * A seat's library figure. Where the viewer may look through more of the deck than its pod shows (its own list, a
 * public list, or tokens its commander makes), it is a button that opens the list; else a plain figure.
 */
export function LibraryChip({ seat, owner, count, density }: LibraryChipProps) {
  const list = useSeatList(seat)
  const [open, setOpen] = useState(false)
  const Icon = ZONE_ICON.library
  const label = zoneLabel('library', count)
  if (!hasMore(list)) {
    return (
      <span
        role="img"
        aria-label={label}
        title={label}
        data-zone="library"
        className={cn(DENSITY[density].chip, 'text-muted-foreground')}
      >
        <Icon aria-hidden="true" className={cn('shrink-0', DENSITY[density].icon)} />
        <span aria-hidden="true" className="tabular-nums">
          {count}
        </span>
      </span>
    )
  }
  const opens = list.whole ? `open ${owner}’s deck list` : `open what is public of ${owner}’s deck`
  return (
    <>
      <Button
        variant={density === 'tile' ? 'ghost' : 'outline'}
        size="xs"
        aria-haspopup="dialog"
        aria-label={`${label}, ${opens}`}
        title={`${label} · Deck list`}
        data-zone="library"
        data-list={list.whole ? 'whole' : 'public'}
        onClick={() => setOpen(true)}
        className={cn('min-w-0 text-xs', density === 'panel' ? 'gap-1 px-1.5' : 'gap-0.5 px-0.5')}
      >
        <Icon aria-hidden="true" className={DENSITY[density].icon} />
        <span aria-hidden="true" className="tabular-nums">
          {count}
        </span>
      </Button>
      <DeckListDialog seat={seat} open={open} onOpenChange={setOpen} />
    </>
  )
}

/** The table menu's buttons, one for each seat with a list to look through; nothing while there is none. */
export function DeckListMenu({ onPick }: { onPick: (seat: PlayerId) => void }) {
  const lists = usePlay((state) => state.deckLists)
  const name = useSeatNamer()
  const viewer = useViewerSeat()
  const shown = lists.filter(hasMore)
  if (shown.length === 0) return null
  return (
    <div role="group" aria-label="Deck lists" className="flex flex-wrap gap-2">
      {shown.map((list) => (
        <Button
          key={list.seat}
          size="sm"
          variant="outline"
          aria-haspopup="dialog"
          data-seat={list.seat}
          onClick={() => onPick(list.seat)}
        >
          {list.seat === viewer ? 'Your deck' : `${name(list.seat)}’s deck`}
        </Button>
      ))}
    </div>
  )
}
