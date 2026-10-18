/**
 * The forecast the table draws, and the one seam where it is decided. The client never works out a combat result: it
 * shows what the engine put in `combat_forecast`, and hides it when it can no longer be right.
 */
import { usePlay } from '@/features/game/gameContext'
import type { CombatForecast, ObjectId } from '@/protocol/generated'
import { usePendingCombat } from './pending'

/** What the forecast says will happen to one creature, when there is something to show. */
export interface CreatureOutlook {
  /** Combat damage it would take; 0 for a creature that dies without any (toughness 0). */
  damage: number
  /** Whether that is enough to destroy it. */
  dies: boolean
}

/**
 * Whether blocks being chosen have overtaken the table's forecast. The forecast describes combat as the table
 * stands, so it fits the answer only while no block is pending. The open roadmap item "Block preview" replaces this
 * with the engine's forecast of the pending answer (`ClientMessage::Preview`).
 */
export function isOutdated(blocks: ReadonlyMap<ObjectId, ObjectId>): boolean {
  return blocks.size > 0
}

/** The forecast to draw for the blocks being chosen: the table's own, or none when blocks make it out of date. */
export function previewFor(
  table: CombatForecast | null,
  blocks: ReadonlyMap<ObjectId, ObjectId>,
): CombatForecast | null {
  return isOutdated(blocks) ? null : table
}

/**
 * THE SEAM: the forecast to draw. Every forecast on screen (the overlay's colours, the marks on cards and panels, the
 * key, the sentences and the outcome in the DeclareBlockers prompt) comes from here. Today it is `previewFor` the
 * table's `combat_forecast` and the blocks being chosen; the "Block preview" item replaces the body with the engine's
 * forecast of the pending answer.
 */
export function useBlockPreview(): CombatForecast | null {
  const table = usePlay((state) => state.view?.combat_forecast ?? null)
  return previewFor(table, usePendingCombat().blocks)
}

/** The outlook of one creature in `forecast`, or null when nothing would happen to it. */
export function creatureOutlook(forecast: CombatForecast | null, id: ObjectId): CreatureOutlook | null {
  const entry = forecast?.creatures.find((creature) => creature.object === id)
  return entry === undefined || (entry.damage === 0 && !entry.dies)
    ? null
    : { damage: entry.damage, dies: entry.dies }
}
