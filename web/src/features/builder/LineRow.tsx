import { EllipsisVerticalIcon, MinusIcon, PlusIcon, Trash2Icon } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { SupportMark, supportEdge } from '@/features/decks/SupportMark'
import { cn } from '@/lib/utils'
import type { DeckZone } from '@/protocol/generated'
import type { Line } from './deckEdit'
import { ManaCost } from './ManaCost'
import { isIssue, reasonLines } from './report'
import type { LineCheck } from './report'
import { ZONE_THE, ZONE_TO, zonesFor } from './zones'

/** What a line can do; each takes the line's zone and card, so the pane passes one set for all lines. */
export interface LineActions {
  increment: (zone: DeckZone, oracleId: string) => void
  decrement: (zone: DeckZone, oracleId: string) => void
  remove: (zone: DeckZone, oracleId: string) => void
  move: (zone: DeckZone, oracleId: string, to: DeckZone, count?: number) => void
}

interface LineRowProps {
  line: Line
  format: string
  /** False when the deck is at its card limit, so another copy cannot be added. */
  room: boolean
  /** What the deck check says of the card. */
  check: LineCheck
  actions: LineActions
}

/**
 * One line of the deck: the copies with minus and plus (the last copy removed takes the line out), the name, the cost,
 * and a menu to move copies to another zone with Remove beside it. Under 640 px the row wraps to two lines. A card the
 * game cannot play by itself has an edge in its state's colour and, under the name, its mark and the reasons the check
 * gives.
 */
export function LineRow({ line, format, room, check, actions }: LineRowProps) {
  const { zone, card, count } = line
  const id = card.oracle_id
  const targets = zonesFor(format).filter((to) => to !== zone)
  const issue = isIssue(check.support)
  const reasons = reasonLines(check.reasons)
  return (
    <li
      data-line={`${zone}:${id}`}
      data-support={check.support}
      className={cn(
        'grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-2 gap-y-1 rounded-lg border-l-4 px-1 py-1 hover:bg-accent sm:grid-cols-[auto_minmax(0,1fr)_auto_auto]',
        issue ? supportEdge(check.support) : 'border-transparent',
      )}
    >
      <div
        role="group"
        aria-label={`${card.name}: ${count} in ${ZONE_THE[zone]}`}
        className="order-3 flex items-center sm:order-none"
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
      <span className="order-1 min-w-0 truncate text-sm sm:order-none" title={card.name}>
        {card.name}
      </span>
      <span className="order-2 justify-self-end sm:order-none">
        <ManaCost cost={card.mana_cost} />
      </span>
      <div className="order-4 flex items-center justify-self-end sm:order-none">
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
          </DropdownMenuContent>
        </DropdownMenu>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={`Remove ${card.name} from ${ZONE_THE[zone]}`}
          onClick={() => actions.remove(zone, id)}
        >
          <Trash2Icon aria-hidden="true" />
        </Button>
      </div>
      {issue ? (
        <div
          data-slot="line-check"
          className="order-5 col-span-full flex items-start gap-2 sm:order-none sm:col-span-3 sm:col-start-2"
        >
          <SupportMark support={check.support} />
          {reasons.length === 0 ? null : (
            <ul
              aria-label={`Why ${card.name} is not verified`}
              className="grid min-w-0 flex-1 gap-1 pt-1 text-xs"
            >
              {reasons.map((reason) => (
                <li key={reason.key} className="text-muted-foreground">
                  {reason.text}
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : null}
    </li>
  )
}
