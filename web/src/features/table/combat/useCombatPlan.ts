import { useMemo } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { CombatView, ObjectId } from '@/protocol/generated'
import { useBlockPreview } from './forecast'
import { usePendingCombat } from './pending'
import type { PendingCombat } from './pending'
import { planLines } from './plan'
import type { LineSpec } from './plan'

const NO_LINES: readonly LineSpec[] = []

/** Every card the combat and the unsent choices name: attackers, blockers and attacked planeswalkers. */
export function cardsInPlay(combat: CombatView | null, pending: PendingCombat): ObjectId[] {
  const ids = new Set<ObjectId>()
  for (const attacker of combat?.attackers ?? []) {
    ids.add(attacker.object)
    for (const blocker of attacker.blockers) ids.add(blocker)
    if ('Planeswalker' in attacker.target) ids.add(attacker.target.Planeswalker)
  }
  for (const [attacker, target] of pending.attacks) {
    ids.add(attacker)
    if ('Planeswalker' in target) ids.add(target.Planeswalker)
  }
  for (const [blocker, attacker] of pending.blocks) {
    ids.add(blocker)
    ids.add(attacker)
  }
  return [...ids]
}

/**
 * The lines to draw for the table as it stands: the declared combat (coloured by the forecast that holds) and the
 * choices a prompt has not sent yet. The same array until something that changes a line changes.
 */
export function useCombatPlan(): readonly LineSpec[] {
  const combat = usePlay((state) => state.view?.combat ?? null)
  const forecast = useBlockPreview()
  const pending = usePendingCombat()
  const cards = useMemo(() => cardsInPlay(combat, pending), [combat, pending])
  const controllers = usePlay(useShallow((state) => cards.map((id) => state.objectsById[id]?.controller)))
  return useMemo(() => {
    const controllerOf = (id: ObjectId) => controllers[cards.indexOf(id)]
    const lines = planLines({ combat, forecast, pending, controllerOf })
    return lines.length === 0 ? NO_LINES : lines
  }, [combat, forecast, pending, cards, controllers])
}
