/** Words for how a game ended: the headline for this seat, and what happened to each player. */
import type { GameResult, LossReason, PlayerId, PlayerView } from '@/protocol/generated'

/** How a player lost, as a phrase that follows their name ("Bo conceded"). */
export function describeLoss(reason: LossReason): string {
  if (typeof reason !== 'string') return `lost: ${reason.Format}`
  switch (reason) {
    case 'ZeroLife':
      return 'ran out of life'
    case 'EmptyLibrary':
      return 'could not draw from an empty library'
    case 'Conceded':
      return 'conceded'
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

/** Headline and detail of a finished game for the seat `me`; `players` may be empty when no view ever arrived. */
export function describeResult(
  result: GameResult,
  me: PlayerId | null,
  name: Names,
  players: readonly PlayerView[],
): Outcome {
  const losses = players.flatMap((player) =>
    player.lost === null ? [] : [`${subject(player.seat, me, name)} ${describeLoss(player.lost)}`],
  )
  const how = sentence(losses)
  if (result === 'Draw') return { headline: 'The game is a draw', detail: how ?? 'Nobody won.' }
  if (result.Winner === me) return { headline: 'You won', detail: how }
  const won = `${name(result.Winner)} won.`
  const lost = me !== null && players.some((player) => player.seat === me && player.lost !== null)
  if (!lost) return { headline: `${name(result.Winner)} won`, detail: how }
  return { headline: 'You lost', detail: how === null ? won : `${won} ${how}` }
}

/** One entry per seat for the results list. */
export function seatOutcomes(
  result: GameResult,
  me: PlayerId | null,
  name: Names,
  players: readonly PlayerView[],
): SeatOutcome[] {
  return players.map((player) => {
    let status = 'Still in'
    if (player.lost !== null) status = `Lost: ${describeLoss(player.lost)}`
    else if (result === 'Draw') status = 'Draw'
    else if (result.Winner === player.seat) status = 'Winner'
    return { seat: player.seat, name: name(player.seat), you: player.seat === me, status, life: player.life }
  })
}
