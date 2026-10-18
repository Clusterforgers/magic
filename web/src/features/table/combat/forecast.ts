/**
 * The forecast the table draws, and the one seam where it is decided. The client never works out a combat result: it
 * shows what the engine said, in `combat_forecast` of the table view or in the reply to a preview, and hides it when it
 * can no longer be right.
 */
import { usePlay } from '@/features/game/gameContext'
import { previewKey } from '@/play/preview'
import type { PreviewState } from '@/play/preview'
import type { Answer, AttackTarget, CombatForecast, DecisionId, ObjectId } from '@/protocol/generated'
import { usePendingCombat } from './pending'
import type { PendingCombat } from './pending'

/** What the forecast says will happen to one creature, when there is something to show. */
export interface CreatureOutlook {
  /** Combat damage it would take; 0 for a creature that dies without any (toughness 0). */
  damage: number
  /** Whether that is enough to destroy it. */
  dies: boolean
}

/** The answer that makes these blocks (blocker to attacker, in the order they were made); null when there are none. */
export function blocksAnswer(blocks: ReadonlyMap<ObjectId, ObjectId>): Answer | null {
  return blocks.size === 0 ? null : { DeclareBlockers: [...blocks] }
}

/** The answer that makes these attacks (attacker to what it attacks, in the order chosen); null when there are none. */
export function attacksAnswer(attacks: ReadonlyMap<ObjectId, AttackTarget>): Answer | null {
  return attacks.size === 0 ? null : { DeclareAttackers: [...attacks] }
}

/**
 * The answer a prompt would send for the choices it has made and not sent, or null when it has made none. A prompt
 * confirms with exactly this answer, which is what makes the engine's preview of it the forecast of the confirm.
 */
export function pendingAnswer(pending: PendingCombat): Answer | null {
  return blocksAnswer(pending.blocks) ?? attacksAnswer(pending.attacks)
}

/**
 * Which forecast to draw. While nothing is being chosen (`choice` is null) it is the table's own, which describes the
 * combat as it stands. While something is, the table's forecast knows nothing of it, so it is the engine's preview of
 * exactly that choice (`choice` is its `previewKey`) for the decision now pending, and nothing until that arrives.
 */
export function previewFor(
  table: CombatForecast | null,
  choice: string | null,
  preview: PreviewState | null,
  pending: DecisionId | null,
): CombatForecast | null {
  if (choice === null) return table
  if (preview === null || preview.decision !== pending || preview.key !== choice) return null
  return preview.forecast
}

/**
 * THE SEAM: the forecast to draw. Every forecast on screen (the overlay's colours, the marks on cards and panels, the
 * key, the sentences and the outcome in the DeclareBlockers and DeclareAttackers prompts) comes from here: the table's
 * `combat_forecast` while no choice is pending, the engine's preview of the choice being made otherwise.
 */
export function useBlockPreview(): CombatForecast | null {
  const table = usePlay((state) => state.view?.combat_forecast ?? null)
  const answer = pendingAnswer(usePendingCombat())
  const choice = answer === null ? null : previewKey(answer)
  // Selecting the forecast itself keeps the same object until the store holds another one.
  const previewed = usePlay((state) => previewFor(null, choice, state.preview, state.pending?.id ?? null))
  return choice === null ? table : previewed
}

/** The outlook of one creature in `forecast`, or null when nothing would happen to it. */
export function creatureOutlook(forecast: CombatForecast | null, id: ObjectId): CreatureOutlook | null {
  const entry = forecast?.creatures.find((creature) => creature.object === id)
  return entry === undefined || (entry.damage === 0 && !entry.dies)
    ? null
    : { damage: entry.damage, dies: entry.dies }
}
