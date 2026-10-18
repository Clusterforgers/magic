/** Which steps can hold a stop; the server refuses a request for any other. */
import type { Step } from '@/protocol/generated'

/** The steps that can hold a stop, in turn order: those where a player gets priority (never untap or cleanup). */
export const STOP_STEPS = [
  'Upkeep',
  'Draw',
  'Main1',
  'BeginCombat',
  'DeclareAttackers',
  'DeclareBlockers',
  'FirstStrikeDamage',
  'CombatDamage',
  'EndCombat',
  'Main2',
  'End',
] as const satisfies readonly Step[]

/** A step that can hold a stop. */
export type StopStep = (typeof STOP_STEPS)[number]

/** Whether `step` can hold a stop. */
export function canHoldStop(step: Step): step is StopStep {
  return (STOP_STEPS as readonly Step[]).includes(step)
}
