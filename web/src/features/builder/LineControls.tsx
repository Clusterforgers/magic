import { EllipsisVerticalIcon, MinusIcon, PlusIcon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import type { CardHit, DeckZone } from '@/protocol/generated'
import type { Line } from './deckEdit'
import { ZONE_THE, ZONE_TO, zonesFor } from './zones'

/** What a line can do; each takes the line's zone and card, so the pane passes one set for all lines. */
export interface LineActions {
  increment: (zone: DeckZone, oracleId: string) => void
  decrement: (zone: DeckZone, oracleId: string) => void
  remove: (zone: DeckZone, oracleId: string) => void
  move: (zone: DeckZone, oracleId: string, to: DeckZone, count?: number) => void
  /** Opens the preview of a card; `opener` is the control focus returns to. */
  preview: (card: CardHit, opener: HTMLElement) => void
}

/** The copies of a line with minus and plus; the last copy removed takes the line out. */
export function Copies({
  line,
  room,
  actions,
  className,
}: {
  line: Line
  /** False when the deck is at its card limit, so another copy cannot be added. */
  room: boolean
  actions: LineActions
  className?: string
}) {
  const { zone, card, count } = line
  const id = card.oracle_id
  return (
    <div
      role="group"
      aria-label={`${card.name}: ${count} in ${ZONE_THE[zone]}`}
      className={cn('flex items-center', className)}
    >
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={`Remove one ${card.name}`}
        onClick={() => actions.decrement(zone, id)}
      >
        <MinusIcon aria-hidden="true" />
      </Button>
      <span data-count className="w-8 text-center text-sm font-medium tabular-nums">
        {count}
      </span>
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={`Add one ${card.name}`}
        aria-disabled={!room || undefined}
        onClick={() => room && actions.increment(zone, id)}
      >
        <PlusIcon aria-hidden="true" />
      </Button>
    </div>
  )
}

/** The menu that moves copies of a line to another zone; `withRemove` adds Remove for a layout without its own button. */
export function MoveMenu({
  line,
  format,
  actions,
  withRemove = false,
}: {
  line: Line
  format: string
  actions: LineActions
  withRemove?: boolean
}) {
  const { zone, card, count } = line
  const id = card.oracle_id
  const targets = zonesFor(format).filter((to) => to !== zone)
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={`Move ${card.name} to another part of the deck`}
          />
        }
      >
        <EllipsisVerticalIcon aria-hidden="true" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuGroup>
          <DropdownMenuLabel>Move to</DropdownMenuLabel>
          {targets.map((to) => (
            <DropdownMenuItem key={to} onClick={() => actions.move(zone, id, to)}>
              {count > 1 ? `All ${count} to ${ZONE_TO[to]}` : `To ${ZONE_TO[to]}`}
            </DropdownMenuItem>
          ))}
          {count > 1
            ? targets.map((to) => (
                <DropdownMenuItem key={`one-${to}`} onClick={() => actions.move(zone, id, to, 1)}>
                  {`One to ${ZONE_TO[to]}`}
                </DropdownMenuItem>
              ))
            : null}
        </DropdownMenuGroup>
        {withRemove ? (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem className="text-destructive" onClick={() => actions.remove(zone, id)}>
              {`Remove from ${ZONE_THE[zone]}`}
            </DropdownMenuItem>
          </>
        ) : null}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
