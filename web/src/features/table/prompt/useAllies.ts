import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { teammates } from '@/lib/teams'

/** The viewer's teammates, without the viewer: empty outside a team mode (Teams, Two-Headed Giant, a Delves party). */
export function useAllies(): readonly number[] {
  return usePlay(
    useShallow((state) => {
      const teams = state.view?.house_rules?.teams
      return state.seat === null || teams == null ? [] : teammates(teams, state.seat)
    }),
  )
}
