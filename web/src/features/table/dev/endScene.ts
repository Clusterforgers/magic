/** `/dev/table?teams=` and `?result=`: the fixture table in two teams, and the fixture game ended with a result. */
import type { GameResult, PlayerView } from '@/protocol/generated'
import type { DevResult, DevTeams } from '../devSearch'
import type { Fixture } from './fixture'

/** The shared life of each team in Two-Headed Giant: the viewer's team, then the other. */
const SHARED_LIFE = [34, 27] as const

/** The table in two teams, even seats against odd ones; in Two-Headed Giant each team's seats share one life total. */
export function withTeams(moment: Fixture, mode: DevTeams): Fixture {
  const seats = moment.view.players.map((player) => player.seat % 2)
  const players =
    mode === 'two_headed_giant'
      ? moment.view.players.map((player) => ({
          ...player,
          life: player.seat % 2 === 0 ? SHARED_LIFE[0] : SHARED_LIFE[1],
        }))
      : moment.view.players
  const house_rules = { ...moment.view.house_rules, teams: { mode, seats } }
  return { ...moment, view: { ...moment.view, players, house_rules } }
}

/** The game's result and the players as they finished: who won, and every other seat lost on life. */
export function endedWith(moment: Fixture, kind: DevResult): { players: PlayerView[]; result: GameResult } {
  const viewer = moment.view.viewer ?? 0
  const teams = moment.view.house_rules?.teams?.seats
  const teamOf = (seat: number): number => teams?.[seat] ?? seat
  const winner = moment.view.players.find((player) => player.seat !== viewer)?.seat ?? viewer
  const result: GameResult =
    kind === 'draw'
      ? 'Draw'
      : kind === 'team'
        ? { Team: teamOf(viewer) }
        : { Winner: kind === 'won' ? viewer : winner }
  const won = (seat: number): boolean =>
    typeof result === 'string'
      ? false
      : 'Team' in result
        ? teamOf(seat) === result.Team
        : seat === result.Winner
  // A draw ends the game for everyone without a loser.
  const players = moment.view.players.map((player) =>
    kind === 'draw' || won(player.seat)
      ? player
      : { ...player, life: Math.min(player.life, 0), lost: 'ZeroLife' as const },
  )
  return { players, result }
}
