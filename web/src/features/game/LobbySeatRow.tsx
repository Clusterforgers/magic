import { Circle, CircleCheck, CircleDashed, TriangleAlertIcon, UserMinus } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { Pill } from '@/components/Pill'
import type { PillTone } from '@/components/Pill'
import { TeamBadge } from '@/components/TeamBadge'
import { Button } from '@/components/ui/button'
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select'
import { teamColour, teamLabel } from '@/lib/teams'
import { tableWarning } from '@/features/decks/legality'
import { cn } from '@/lib/utils'
import { deckWords } from './deckWords'
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
  /** This seat made the game. */
  host?: boolean
  /** The seat's team at a table with teams: its number, and for the host the teams it may move to. */
  team?: {
    team: number
    /** The host's choice of teams; absent for everyone else, who sees the team only. */
    change?: {
      choices: readonly number[]
      /** A request is out, so the choice waits. */
      disabled: boolean
      onChange(team: number): void
    }
  }
  /** The host's way to take this player out: present only on a seat that may be removed. */
  remove?: {
    /** A request is out, so the button waits. */
    disabled: boolean
    /** Asks the question; gets the button, where focus goes back to on No. */
    onPress(opener: HTMLElement): void
  }
}

/**
 * One seat of the lobby: a dot for whether a client is connected, the seat and its name (long ones wrap), a mark on
 * this page's own seat, and in words whether the seat is ready, not ready yet or not here. The words carry
 * everything the dot and the icon show. The host's rows of joined players also have a Remove button.
 */
export function LobbySeatRow({ row, account, host = false, team, remove }: LobbySeatRowProps) {
  const state = seatState(row)
  const { label, Icon, tone } = STATES[state]
  return (
    <li
      data-seat-row={row.seat}
      data-state={state}
      data-connected={row.connected || undefined}
      data-team={team?.team}
      className={cn(
        'flex items-start justify-between gap-3 px-4 py-3 text-sm',
        // A team's colour on the row's edge groups teammates at a glance; the label says it in words.
        team !== undefined && cn('border-l-4', teamColour(team.team).edge),
      )}
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
        <span className="grid min-w-0 gap-1">
          <span className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
            <span className="shrink-0 text-muted-foreground">Seat {row.seat + 1}</span>
            <span data-slot="seat-name" className="min-w-0 font-medium [overflow-wrap:anywhere]">
              {row.name}
            </span>
            {row.you ? <Pill>you</Pill> : null}
            {host ? <Pill data-slot="seat-host">host</Pill> : null}
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
          {team === undefined ? null : team.change === undefined ? (
            <TeamBadge team={team.team} />
          ) : (
            <TeamChoice seat={row.seat} name={row.name} team={team.team} change={team.change} />
          )}
          {/* What the whole table is told of the deck: its name, format and a count, never a card. An open seat has none to tell. */}
          {!row.named || row.deck === undefined ? null : (
            <span
              data-slot="seat-deck"
              data-deck={row.deck === null ? 'none' : 'chosen'}
              className="min-w-0 text-xs text-muted-foreground [overflow-wrap:anywhere]"
            >
              <span className="sr-only">Deck: </span>
              {row.deck === null ? 'no deck yet' : deckWords(row.deck)}
            </span>
          )}
          {/* Everyone sees that a seat's deck breaks the table's rules, as a count; only its owner learns which. */}
          {!row.named || row.deck == null || row.deck.warnings === 0 ? null : (
            <span
              data-slot="seat-deck-warnings"
              className="flex min-w-0 items-start gap-1 text-xs font-medium text-warning"
            >
              <TriangleAlertIcon aria-hidden="true" className="mt-px size-3.5 shrink-0" />
              <span className="min-w-0 [overflow-wrap:anywhere]">{tableWarning(row.deck.warnings)}</span>
            </span>
          )}
        </span>
      </span>
      {/* Remove comes first so the state pills end at the same edge in every row; on a phone, where the name needs the width, the pill is above it. */}
      <span className="flex shrink-0 flex-col-reverse items-end gap-1">
        {remove === undefined ? null : (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            data-slot="remove-player"
            disabled={remove.disabled}
            aria-label={`Remove ${row.name} from the game`}
            onClick={(event) => remove.onPress(event.currentTarget)}
          >
            <UserMinus aria-hidden="true" />
            Remove
          </Button>
        )}
        <Pill tone={tone} data-slot="seat-state" className="shrink-0">
          <Icon aria-hidden="true" />
          {label}
          {state === 'ready' && !row.connected ? <span className="font-normal">(away)</span> : null}
        </Pill>
      </span>
    </li>
  )
}

/** The host's choice of a seat's team: a small select labelled with the seat, next to the team's colour. */
function TeamChoice({
  seat,
  name,
  team,
  change,
}: {
  seat: number
  name: string
  team: number
  change: NonNullable<NonNullable<LobbySeatRowProps['team']>['change']>
}) {
  const id = `seat-${seat}-team`
  return (
    <span className="flex items-center gap-2">
      <span aria-hidden="true" className={cn('size-2.5 shrink-0 rounded-full', teamColour(team).dot)} />
      <label htmlFor={id} className="sr-only">
        Team of {name}
      </label>
      <NativeSelect
        id={id}
        size="sm"
        data-team-select=""
        value={String(team)}
        disabled={change.disabled}
        onChange={(event) => change.onChange(Number(event.target.value))}
      >
        {change.choices.map((choice) => (
          <NativeSelectOption key={choice} value={String(choice)}>
            {teamLabel(choice)}
          </NativeSelectOption>
        ))}
      </NativeSelect>
    </span>
  )
}
