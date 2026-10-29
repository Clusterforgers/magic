/** The table's regions with the board rows split by what each must hold (`boardSplit.ts`). */
import { useContext, useMemo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { seatCounts, splitRegions } from './boardSplit'
import type { LaneCounts } from './laneFit'
import type { Regions, SeatPlan } from './layout'
import { STACKS_BY_DEFAULT, TableOptionsContext } from './tableOptions'

const parse = (key: string): LaneCounts => {
  const [lands = 0, creatures = 0, other = 0, tuckedLands = 0, tuckedCreatures = 0, tuckedOther = 0] = key
    .split(',')
    .map(Number)
    const counts: LaneCounts = { lands, creatures, other }
  if (tuckedLands + tuckedCreatures + tuckedOther === 0) return counts
  return { ...counts, tucked: { lands: tuckedLands, creatures: tuckedCreatures, other: tuckedOther } }
}

/**
 * `regions` with the opponents' row and yours sized by `splitRegions`. The counts are read as one string, so a tap or
 * a change inside a stack redraws nothing here.
 */
export function useBoardSplit(regions: Regions, plan: SeatPlan, reserve = 0): Regions {
  const grouped = useContext(TableOptionsContext).stacks ?? STACKS_BY_DEFAULT
  const key = usePlay((state) =>
    [plan.viewer, ...plan.boards]
      .map((seat) => {
        const { lands, creatures, other, tucked } = seatCounts(state, seat, grouped)
        return `${lands},${creatures},${other},${tucked?.lands ?? 0},${tucked?.creatures ?? 0},${tucked?.other ?? 0}`
      })
      .join('|'),
  )
  return useMemo(() => {
    const [own = '0,0,0', ...theirs] = key.split('|')
    return splitRegions(regions, plan, { own: parse(own), theirs: theirs.map(parse) }, reserve)
  }, [key, regions, plan, reserve])
}
