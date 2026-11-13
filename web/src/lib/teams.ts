/** Team play as plain data: the team of a seat, its label, colour and members, the default teams and the result. */
import type { GameResult, TeamMode, Teams } from '@/protocol/generated'

/** How the table plays: every player alone, or one of the two team modes. */
export type GameMode = 'free_for_all' | TeamMode

/** The words of each mode, for the create form, the lobby and the table. */
export const MODE_WORDS: Record<GameMode, { name: string; words: string }> = {
  free_for_all: { name: 'Free-for-all', words: 'Every player for themself' },
  two_headed_giant: {
    name: 'Two-Headed Giant',
    words: 'Four players in two teams of two; teammates sit side by side and share one life total',
  },
  teams: { name: 'Teams', words: 'Two teams of equal size; teammates sit apart and win together' },
}

/** The seats a Two-Headed Giant table has. */
export const GIANT_SEATS = 4

/** The fewest seats of a team game: two teams of two. */
export const MIN_TEAM_SEATS = 4

/** How many team colours there are; a later team takes the colour of an earlier one, and its label tells them apart. */
export const TEAM_COLOURS = 4

// Whole class names, so that Tailwind finds them in the source.
const TEXT = ['text-team-1', 'text-team-2', 'text-team-3', 'text-team-4'] as const
const DOT = ['bg-team-1', 'bg-team-2', 'bg-team-3', 'bg-team-4'] as const
const EDGE = ['border-team-1', 'border-team-2', 'border-team-3', 'border-team-4'] as const

/** The classes of team `team`'s colour: its text, a filled dot and an edge. */
export function teamColour(team: number): { text: string; dot: string; edge: string } {
  const index = ((team % TEAM_COLOURS) + TEAM_COLOURS) % TEAM_COLOURS
  return { text: TEXT[index] ?? TEXT[0], dot: DOT[index] ?? DOT[0], edge: EDGE[index] ?? EDGE[0] }
}

/** "Team 1" for team 0: teams are counted from 0 on the wire and from 1 in words. */
export const teamLabel = (team: number): string => `Team ${team + 1}`

/** The team of `seat`, or null when the table has no teams or does not list the seat. */
export function teamOf(teams: Teams | null | undefined, seat: number): number | null {
  return teams?.seats[seat] ?? null
}

/** The seats of team `team` in seat order. */
export function teamMembers(teams: Teams | null | undefined, team: number): number[] {
  if (teams == null) return []
  return teams.seats.flatMap((each, seat) => (each === team ? [seat] : []))
}

/** The seats on the same team as `seat`, without it. */
export function teammates(teams: Teams | null | undefined, seat: number): number[] {
  const team = teamOf(teams, seat)
  return team === null ? [] : teamMembers(teams, team).filter((other) => other !== seat)
}

/**
 * The seat whose pod shows `seat`'s life: in Two-Headed Giant teammates share one life total, shown once on the
 * team's first seat; everywhere else each seat shows its own.
 */
export function lifeHolder(teams: Teams | null | undefined, seat: number): number {
  if (teams?.mode !== 'two_headed_giant') return seat
  const team = teamOf(teams, seat)
  return team === null ? seat : (teamMembers(teams, team)[0] ?? seat)
}

/** How many teams the table has: one more than the highest team number. */
export const teamCount = (teams: Teams | null | undefined): number =>
  teams == null || teams.seats.length === 0 ? 0 : Math.max(...teams.seats) + 1

/** The mode of a table: its teams' mode, or free-for-all without teams. */
export const modeOf = (teams: Teams | null | undefined): GameMode => teams?.mode ?? 'free_for_all'

/**
 * The teams a new table of `count` seats starts with, as the engine's `Teams::even` makes them: Two-Headed Giant seats
 * teammates side by side (0, 0, 1, 1), Teams alternates two teams (0, 1, 0, 1, ...).
 */
export function defaultTeams(mode: TeamMode, count: number): Teams {
  const size = mode === 'two_headed_giant' ? 2 : Math.max(1, Math.floor(count / 2))
  const seats = Array.from({ length: count }, (_, seat) =>
    mode === 'two_headed_giant' ? Math.floor(seat / size) : seat % 2,
  )
  return { mode, seats }
}

/** Why `count` seats cannot play `mode`, in words; null when they can. */
export function seatsProblem(mode: GameMode, count: number): string | null {
  if (mode === 'two_headed_giant' && count !== GIANT_SEATS) return 'Two-Headed Giant needs exactly 4 seats.'
  if (mode === 'teams' && (count < MIN_TEAM_SEATS || count % 2 !== 0)) {
    return 'Teams needs an even number of seats, 4 or more.'
  }
  return null
}

/**
 * Why the game cannot start with `teams` (`docs/server.md`): fewer than two teams, or a Two-Headed Giant that is not two
 * teams of two. The lobby keeps any grouping while players choose; the server checks it at the start, after numbering
 * the teams without gaps, as this does.
 */
export function teamsProblem(chosen: Teams): string | null {
  const teams = { ...chosen, seats: compact(chosen.seats) }
  const count = teamCount(teams)
  if (count < 2) return 'A team game needs at least two teams.'
  if (teams.mode === 'two_headed_giant') {
    const even = teams.seats.length === GIANT_SEATS && count === 2 && teamMembers(teams, 0).length === 2
    if (!even) return 'Two-Headed Giant needs two teams of two.'
  }
  return null
}

/** Team numbers renumbered from 0 in the order of their numbers, so that a team left empty leaves no gap. */
function compact(seats: readonly number[]): number[] {
  const used = [...new Set(seats)].sort((a, b) => a - b)
  return seats.map((team) => used.indexOf(team))
}

/** The teams with `seat` on team `team` and every other seat where it was: the lobby keeps exactly what is chosen. */
export function withSeatTeam(teams: Teams, seat: number, team: number): Teams {
  const from = teams.seats[seat]
  if (from === undefined || from === team) return teams
  const seats = [...teams.seats]
  seats[seat] = team
  return { ...teams, seats }
}

/**
 * The teams of the lobby's columns, each one a seat may choose: two in Two-Headed Giant, and in Teams one for every two
 * seats (at least two), and any team a seat already holds.
 */
export function teamChoices(teams: Teams): number[] {
  const columns = teams.mode === 'two_headed_giant' ? 2 : Math.max(2, Math.floor(teams.seats.length / 2))
  const count = Math.max(columns, teamCount(teams))
  return Array.from({ length: count }, (_, team) => team)
}

/** The seats that won `result`: the winner, the members of the winning team, or none for a draw. */
export function winningSeats(result: GameResult | null, teams: Teams | null | undefined): number[] {
  if (result === null || result === 'Draw') return []
  if ('Winner' in result) return [result.Winner]
  return teamMembers(teams, result.Team)
}

/** "Ann", "Ann & Bo", "Ann, Bo & Cy": the players of a team. */
export function memberNames(names: readonly string[]): string {
  if (names.length <= 1) return names.join('')
  return `${names.slice(0, -1).join(', ')} & ${names.at(-1) ?? ''}`
}

/** "Team 1 (Ann & Bo)", or "Team 1" when the members are not known. */
export function teamName(
  teams: Teams | null | undefined,
  team: number,
  name: (seat: number) => string,
): string {
  const members = teamMembers(teams, team)
  return members.length === 0 ? teamLabel(team) : `${teamLabel(team)} (${memberNames(members.map(name))})`
}

/**
 * Whether it is `seat`'s turn while `active` is the active player: the active player's own, and in Two-Headed Giant
 * their teammate's as well, since a team shares its turn (CR 805.3). In Teams each player takes their own turn.
 */
export function sharesTurn(teams: Teams | null | undefined, seat: number, active: number): boolean {
  if (seat === active) return true
  if (teams?.mode !== 'two_headed_giant') return false
  const team = teamOf(teams, seat)
  return team !== null && team === teamOf(teams, active)
}
