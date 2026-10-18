import { Circle, CircleCheck, CircleDashed } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import { seatState } from './lobbyModel'
import type { LobbyRow, SeatState } from './lobbyModel'

interface Words {
  label: string
  Icon: LucideIcon
  icon: string
  pill: string
}

// Whole class names, so Tailwind can see them.
const STATES: Record<SeatState, Words> = {
  ready: { label: 'Ready', Icon: CircleCheck, icon: 'text-mana-green', pill: 'bg-mana-green/15 font-medium' },
  waiting: { label: 'Not ready yet', Icon: Circle, icon: 'text-muted-foreground', pill: 'bg-muted' },
  away: {
    label: 'Not here',
    Icon: CircleDashed,
    icon: 'text-muted-foreground',
    pill: 'text-muted-foreground',
  },
}

interface LobbySeatRowProps {
  row: LobbyRow
  /** The username of the account sitting at the seat, when the signed-in user's list says so. */
  account: string | null
}

/**
 * One seat of the lobby: a dot for whether a client is connected, the seat and its name (long ones wrap), a mark on
 * this page's own seat, and in words whether the seat is ready, not ready yet or not here. The words carry
 * everything the dot and the icon show.
 */
export function LobbySeatRow({ row, account }: LobbySeatRowProps) {
  const state = seatState(row)
  const { label, Icon, icon, pill } = STATES[state]
  return (
    <li
      data-seat-row={row.seat}
      data-state={state}
      data-connected={row.connected || undefined}
      className="flex items-start justify-between gap-3 px-3 py-2.5 text-sm"
    >
      <span className="flex min-w-0 items-start gap-2.5">
        <span
          data-slot="connected-dot"
          className={cn(
            'mt-[0.3rem] inline-block size-2.5 shrink-0 rounded-full ring-1 ring-inset',
            row.connected ? 'bg-mana-green ring-foreground/30' : 'bg-transparent ring-muted-foreground/60',
          )}
        >
          <span className="sr-only">{row.connected ? 'Connected' : 'Not connected'}</span>
        </span>
        <span className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
          <span className="shrink-0 text-muted-foreground">Seat {row.seat + 1}</span>
          <span data-slot="seat-name" className="min-w-0 font-medium [overflow-wrap:anywhere]">
            {row.name}
          </span>
          {row.you ? <Badge variant="secondary">you</Badge> : null}
          {account === null ? null : (
            <span
              data-slot="seat-account"
              className="min-w-0 text-xs text-muted-foreground [overflow-wrap:anywhere]"
            >
              signed in as {account}
            </span>
          )}
          {row.named ? null : <span className="text-xs text-muted-foreground">not joined yet</span>}
        </span>
      </span>
      <span
        data-slot="seat-state"
        className={cn('flex shrink-0 items-center gap-1.5 rounded-full px-2 py-0.5 text-xs', pill)}
      >
        <Icon aria-hidden="true" className={cn('size-4', icon)} />
        {label}
        {state === 'ready' && !row.connected ? (
          <span className="font-normal text-muted-foreground">(away)</span>
        ) : null}
      </span>
    </li>
  )
}
