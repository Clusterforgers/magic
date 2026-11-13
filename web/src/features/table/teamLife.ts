/** The team of a seat at the table and where its life is shown, from the house rules of the table view. */
import { usePlay } from '@/features/game/gameContext'
import { lifeHolder, teamOf, teammates } from '@/lib/teams'
import type { PlayerId, Teams } from '@/protocol/generated'
import { useSeatNamer } from './names'

/** A seat's team as a pod draws it. */
export interface SeatTeam {
  /** The team, counted from 0. */
  team: number
  /** `own` for a life of its own; in Two-Headed Giant `team` on the pod that shows the shared total, `teammate` on the other. */
  life: 'own' | 'team' | 'teammate'
  /** The names of the teammates, for the words of a shared life. */
  mates: string[]
  /** Whether the pod shows the team chip; not in a fight against a boss, where the sides are told by place. */
  badge: boolean
}

/** The team of `seat` in the table's house rules, or null at a table without teams. */
export function seatTeam(
  teams: Teams | null | undefined,
  seat: PlayerId,
  name: (seat: PlayerId) => string,
  bossFight = false,
): SeatTeam | null {
  const team = teamOf(teams, seat)
  if (team === null) return null
  const life =
    teams?.mode !== 'two_headed_giant' ? 'own' : lifeHolder(teams, seat) === seat ? 'team' : 'teammate'
  return { team, life, mates: teammates(teams, seat).map(name), badge: !bossFight }
}

/** `seatTeam` for the live table. */
export function useSeatTeam(seat: PlayerId): SeatTeam | null {
  const teams = usePlay((state) => state.view?.house_rules?.teams)
  const bossFight = usePlay((state) => state.view?.players.some((player) => player.boss != null) === true)
  const name = useSeatNamer()
  return seatTeam(teams, seat, name, bossFight)
}
