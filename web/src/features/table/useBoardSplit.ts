/** The table's regions with the board rows split by what each must hold (`boardSplit.ts`). */
import { useContext, useLayoutEffect, useMemo, useRef } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { foldsLands, seatCounts, splitRegions } from './boardSplit'
import type { HeldRows } from './boardSplit'
import { handIdle } from './handIdle'
import type { LaneCounts } from './laneFit'
import { useLandsShown } from './landPrefs'
import type { Regions, SeatPlan } from './layout'
import { handSeat } from './selectors'
import { FOLD_LANDS_BY_DEFAULT, STACKS_BY_DEFAULT, TableOptionsContext } from './tableOptions'

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
 * a change inside a stack redraws nothing here. In boss mode the allies' boards and whether your hand has a card to
 * play are read too (the digit before the `#`).
 */
export function useBoardSplit(
  regions: Regions,
  plan: SeatPlan,
  reserve = 0,
  head = 0,
  overhang = reserve,
): Regions {
  const options = useContext(TableOptionsContext)
  const grouped = options.stacks ?? STACKS_BY_DEFAULT
  const shown = useLandsShown(regions.phone)
  // Lands laid out take their lane in every board; only boards that fold them leave it out.
  const folded = !shown && foldsLands(plan, options.foldLands ?? FOLD_LANDS_BY_DEFAULT, regions.phone)
  const boss = plan.shape === 'boss'
  const key = usePlay((state) => {
    const seats = boss ? [plan.viewer, ...plan.boards, ...plan.allies] : [plan.viewer, ...plan.boards]
    const counts = seats
      .map((seat) => {
        const { lands, creatures, other, tucked } = seatCounts(state, seat, grouped)
        // Folded lands take no lane of a board.
        const laid = folded ? 0 : lands
        return `${laid},${creatures},${other},${tucked?.lands ?? 0},${tucked?.creatures ?? 0},${tucked?.other ?? 0}`
      })
      .join('|')
    return `${boss && !handIdle(state, handSeat(state, plan.viewer)) ? 1 : 0}#${counts}`
  })
  // The rows drawn last: the next split keeps them while they serve (`holdSplit`).
  const held = useRef<HeldRows | null>(null)
  const split = useMemo(() => {
    const [ready = '', rest = ''] = key.split('#')
    const [own = '0,0,0', ...others] = rest.split('|')
    const theirs = others.slice(0, plan.boards.length).map(parse)
    const allies = boss ? others.slice(plan.boards.length).map(parse) : undefined
    return splitRegions(
      regions,
      plan,
      {
        own: parse(own),
        theirs,
        ...(allies === undefined ? {} : { allies, ownReady: ready === '1' }),
      },
      reserve,
      head,
      overhang,
      held.current,
    )
  }, [key, regions, plan, reserve, head, overhang, boss])
  useLayoutEffect(() => {
    held.current = { opponents: split.opponents, own: split.own }
  }, [split])
  return split
}
