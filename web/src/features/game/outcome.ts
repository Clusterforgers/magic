/** Words for how a game ended: the headline for this seat, and what happened to each player. */
import { memberNames, teamLabel, teamMembers, teamOf, winningSeats } from '@/lib/teams'
import type { GameResult, LossReason, PlayerId, PlayerView, Teams } from '@/protocol/generated'

/** How a player lost, as a phrase that follows their name ("Bo conceded"). */
export function describeLoss(reason: LossReason): string {
  if (typeof reason !== 'string') return `lost: ${reason.Format}`
  switch (reason) {
    case 'ZeroLife':
      return 'ran out of life'
    case 'EmptyLibrary':
      return 'could not draw from an empty library'
    case 'Poison':
      return 'had too many poison counters'
    case 'Conceded':
      return 'conceded'
    case 'CommanderDamage':
      return 'lost to commander damage'
    case 'OpponentReachedLife':
      return 'lost to a player at the winning life total'
    case 'LeaderFell':
      return 'fell with their leader'
  }
}

/** What a screen needs to say about one seat at the end. */
export interface SeatOutcome {
  seat: PlayerId
  name: string
  you: boolean
  /** "Winner", "Draw", the way the seat lost, or "Still in" when the game ended around them. */
  status: string
  life: number
  /** The seat's team, or null at a table without teams. */
  team: number | null
}

/** The result, from the point of view of one seat. */
export interface Outcome {
  headline: string
  /** What happened, in a sentence or two, or null when there is nothing to add. */
  detail: string | null
}

type Names = (seat: PlayerId) => string

const subject = (seat: PlayerId, me: PlayerId | null, name: Names): string =>
  seat === me ? 'You' : name(seat)

// "A", "A and B", "A, B and C", with a full stop; null for nothing.
function sentence(parts: readonly string[]): string | null {
  if (parts.length === 0) return null
  const head = parts.slice(0, -1)
  const tail = parts.slice(-1)
  return `${head.length === 0 ? '' : `${head.join(', ')} and `}${tail.join('')}.`
}

/** "Team 1 wins — Ann & Bo", or "Team 1 wins" when the members are not known. */
export function teamWinText(team: number, teams: Teams | null | undefined, name: Names): string {
  const members = teamMembers(teams, team)
  const words = `${teamLabel(team)} wins`
  return members.length === 0 ? words : `${words} — ${memberNames(members.map(name))}`
}

/**
 * Headline and detail of a finished game for the seat `me`; `players` may be empty when no view ever arrived. A team's
 * win names the team and its players, and is "Your team won" for each of them.
 */
export function describeResult(
  result: GameResult,
  me: PlayerId | null,
  name: Names,
  players: readonly PlayerView[],
  teams?: Teams | null,
): Outcome {
  const losses = players.flatMap((player) =>
    player.lost === null ? [] : [`${subject(player.seat, me, name)} ${describeLoss(player.lost)}`],
  )
  const how = sentence(losses)
  if (result === 'Draw') return { headline: 'The game is a draw', detail: how ?? 'Nobody won.' }
  if ('Team' in result) {
    const won = `${teamWinText(result.Team, teams, name)}.`
    const detail = how === null ? won : `${won} ${how}`
    if (me !== null && winningSeats(result, teams).includes(me)) return { headline: 'Your team won', detail }
    const mine = me === null ? null : teamOf(teams, me)
    if (mine !== null) return { headline: 'You lost', detail }
    return { headline: won.slice(0, -1), detail: how }
  }
  if (result.Winner === me) return { headline: 'You won', detail: how }
  const won = `${name(result.Winner)} won.`
  const lost = me !== null && players.some((player) => player.seat === me && player.lost !== null)
  if (!lost) return { headline: `${name(result.Winner)} won`, detail: how }
  return { headline: 'You lost', detail: how === null ? won : `${won} ${how}` }
}

/** The shape of an ending, for the picture beside the headline: won or lost on the board, drawn, or conceded. */
export type EndKind = 'won' | 'lost' | 'drawn' | 'conceded'

/**
 * Which ending this game was. It ended by concession when every seat but the winner is out and one of them gave up;
 * a seat that leaves a running game concedes, so a leave is a concession too. Without a view there is no loss to
 * read, so the result alone says won, lost or drawn.
 */
export function endKind(
  result: GameResult,
  me: PlayerId | null,
  players: readonly PlayerView[],
  teams?: Teams | null,
): EndKind {
  const winners = winningSeats(result, teams)
  const others = players.filter((player) => !winners.includes(player.seat))
  const conceded =
    result !== 'Draw' &&
    others.length > 0 &&
    others.every((player) => player.lost !== null) &&
    others.some((player) => player.lost === 'Conceded')
  if (conceded) return 'conceded'
  if (result === 'Draw') return 'drawn'
  return me !== null && winners.includes(me) ? 'won' : 'lost'
}

/** One entry per seat for the results list. */
export function seatOutcomes(
  result: GameResult,
  me: PlayerId | null,
  name: Names,
  players: readonly PlayerView[],
  teams?: Teams | null,
): SeatOutcome[] {
  const winners = winningSeats(result, teams)
  return players.map((player) => {
    let status = 'Still in'
    // A team wins together, so a member who fell along the way is a winner too.
    if (result !== 'Draw' && 'Team' in result && winners.includes(player.seat)) status = 'Winner'
    else if (player.lost !== null) status = `Lost: ${describeLoss(player.lost)}`
    else if (result === 'Draw') status = 'Draw'
    else if (winners.includes(player.seat)) status = 'Winner'
    return {
      seat: player.seat,
      name: name(player.seat),
      you: player.seat === me,
      status,
      life: player.life,
      team: teamOf(teams, player.seat),
    }
  })
}
