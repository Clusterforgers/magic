import { EllipsisVerticalIcon, HandIcon, MinusIcon, PlusIcon } from 'lucide-react'
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
import { BY_HAND_TITLE } from '@/lib/byHand'
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
  /** Marks a card "play by hand" (`on`) in every zone, or gives it back to the rules. */
  byHand: (oracleId: string, on: boolean) => void
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

/**
 * The toggle that has the table play a card by hand whatever its rules, for a card that misbehaves: pressed, the
 * game hosts every copy of it as a manual card and the check counts it as played by hand.
 */
export function ByHandToggle({ line, actions }: { line: Line; actions: LineActions }) {
  const { card } = line
  const on = line.byHand === true
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      aria-pressed={on}
      aria-label={`Play ${card.name} by hand`}
      title={on ? `${BY_HAND_TITLE} by your choice` : 'Play by hand'}
      data-slot="line-by-hand"
      className={cn(on && 'bg-support-manual/12 text-support-manual hover:text-support-manual')}
      onClick={() => actions.byHand(card.oracle_id, !on)}
    >
      <HandIcon aria-hidden="true" />
    </Button>
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
  const targets = zonesFor(format, card.type_line).filter((to) => to !== zone)
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
