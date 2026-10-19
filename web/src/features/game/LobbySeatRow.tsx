import { Circle, CircleCheck, CircleDashed } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { Pill } from '@/components/Pill'
import type { PillTone } from '@/components/Pill'
import { cn } from '@/lib/utils'
import { seatState } from './lobbyModel'
import type { LobbyRow, SeatState } from './lobbyModel'

interface Words {
  label: string
  Icon: LucideIcon
  tone: PillTone
}

const STATES: Record<SeatState, Words> = {
  ready: { label: 'Ready', Icon: CircleCheck, tone: 'success' },
  waiting: { label: 'Not ready yet', Icon: Circle, tone: 'neutral' },
  away: { label: 'Not here', Icon: CircleDashed, tone: 'neutral' },
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
  const { label, Icon, tone } = STATES[state]
  return (
    <li
      data-seat-row={row.seat}
      data-state={state}
      data-connected={row.connected || undefined}
      className="flex items-start justify-between gap-3 px-4 py-3 text-sm"
    >
      <span className="flex min-w-0 items-start gap-3">
        {/* One line tall, so the dot sits in the middle of the first line however long the name grows. */}
        <span className="flex h-5 shrink-0 items-center">
          <span
            data-slot="connected-dot"
            className={cn(
              'inline-block size-2.5 rounded-full border',
              row.connected ? 'border-success bg-success' : 'border-border-strong bg-transparent',
            )}
          >
            <span className="sr-only">{row.connected ? 'Connected' : 'Not connected'}</span>
          </span>
        </span>
        <span className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
          <span className="shrink-0 text-muted-foreground">Seat {row.seat + 1}</span>
          <span data-slot="seat-name" className="min-w-0 font-medium [overflow-wrap:anywhere]">
            {row.name}
          </span>
          {row.you ? <Pill>you</Pill> : null}
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
      <Pill tone={tone} data-slot="seat-state" className="shrink-0">
        <Icon aria-hidden="true" />
        {label}
        {state === 'ready' && !row.connected ? <span className="font-normal">(away)</span> : null}
      </Pill>
    </li>
  )
}
