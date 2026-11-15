/** The attacks at a seat, read from the combat and the choices not sent yet: the badges and the attacked mark draw them. */
import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId, PlayerId } from '@/protocol/generated'
import { usePendingCombat } from './pending'
import { attackGroups } from './plan'
import type { AttackGroup } from './plan'

/** The attacks at one seat and the names of the cards that make them. */
export interface SeatAttacks {
  groups: readonly AttackGroup[]
  cardName: (id: ObjectId) => string
}

const NOTHING: SeatAttacks = { groups: [], cardName: () => '' }

/** The attacks at a player, declared and still being chosen, grouped by the attacking seat (`attackGroups`). */
export function useSeatAttacks(seat: PlayerId): SeatAttacks {
  const combat = usePlay((state) => state.view?.combat ?? null)
  const pending = usePendingCombat()
  const attackers = useMemo(() => {
    const ids: ObjectId[] = []
    for (const attacker of combat?.attackers ?? []) ids.push(attacker.object)
    for (const [attacker] of pending.attacks) ids.push(attacker)
    return ids
  }, [combat, pending])
  const facts = usePlay(
    useShallow((state) =>
      attackers.flatMap((id) => [state.objectsById[id]?.controller ?? -1, state.objectsById[id]?.name ?? '']),
    ),
  )
  return useMemo(() => {
    const controllerOf = (id: ObjectId): PlayerId | undefined => {
      const seen = facts[attackers.indexOf(id) * 2]
      return typeof seen === 'number' && seen >= 0 ? seen : undefined
    }
    const groups = attackGroups(combat, pending, controllerOf).get(seat) ?? []
    if (groups.length === 0) return NOTHING
    return { groups, cardName: (id) => String(facts[attackers.indexOf(id) * 2 + 1] ?? '') }
  }, [combat, pending, facts, attackers, seat])
}

/** Whether any attack is aimed at the seat, so that its pod or tile can wear the mark of an attacked seat. */
export function useAttacked(seat: PlayerId): boolean {
  return useSeatAttacks(seat).groups.length > 0
}

/**
 * The inner edge of a pod or tile under attack, in every layout. An outline, since the pod's own shadow would hide a
 * ring; an outline that aims an attack (`defenderOutline`) comes after it and wins.
 */
export const ATTACKED_EDGE = 'outline-solid outline-3 -outline-offset-3 outline-destructive'
