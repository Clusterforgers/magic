import type { ReactNode } from 'react'
import { TeamBadge } from '@/components/TeamBadge'
import { memberNames, teamChoices, teamMembers } from '@/lib/teams'
import { cn } from '@/lib/utils'
import type { Teams } from '@/protocol/generated'
import type { LobbyRow } from './lobbyModel'

/** What `TeamColumns` takes. */
export interface TeamColumnsProps {
  /** The teams shown. */
  teams: Teams
  /** The seats of the lobby. */
  rows: readonly LobbyRow[]
  /** Draws one seat's row; its team select moves it to another column. */
  renderRow(row: LobbyRow): ReactNode
}

/**
 * The seats of a team lobby in one column per team a seat may choose, each headed by the team's label and its players.
 * A team nobody chose yet shows as an empty column, so every choice is in plain sight; side by side where the seat
 * panel is wide enough for whole rows, else one above the other.
 */
export function TeamColumns({ teams, rows, renderRow }: TeamColumnsProps) {
  const columns = teamChoices(teams)
  return (
    <div data-slot="team-columns" className="@container">
      <div className={cn('grid gap-3', columns.length >= 3 ? '@5xl:grid-cols-3' : '@3xl:grid-cols-2')}>
        {columns.map((team) => {
          const seats = teamMembers(teams, team)
          const members = rows.filter((row) => seats.includes(row.seat))
          const names = memberNames(members.map((row) => row.name).filter((name) => name !== ''))
          return (
            <section key={team} data-team-column={team} aria-label={`Team ${team + 1}`} className="min-w-0">
              <header className="mb-2 flex min-w-0 items-center gap-2">
                <TeamBadge team={team} />
                <span className="truncate text-sm text-muted-foreground">
                  {members.length === 0 ? 'Nobody yet' : names || `${members.length} open`}
                </span>
              </header>
              {members.length === 0 ? (
                <p className="rounded-lg border border-dashed px-4 py-3 text-sm text-muted-foreground">
                  Choose this team in a seat&apos;s team select.
                </p>
              ) : (
                <ul aria-label={`Seats of Team ${team + 1}`} className="divide-y rounded-lg border">
                  {members.map((row) => renderRow(row))}
                </ul>
              )}
            </section>
          )
        })}
      </div>
    </div>
  )
}
