/**
 * The forecast the table draws, and the one seam where it is decided. The client never works out a combat result: it
 * shows what the engine put in `combat_forecast`, and hides it when it can no longer be right.
 */
import { useSyncExternalStore } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import type { CombatForecast, ObjectId, PlayerForecast, PlayerId } from '@/protocol/generated'
import { usePendingCombat, usePendingStore } from './pending'

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

/** THE SEAM: the forecast to draw for the blocks being chosen. Every forecast on screen is decided here. */
export function previewFor(
  table: CombatForecast | null,
  blocks: ReadonlyMap<ObjectId, ObjectId>,
): CombatForecast | null {
  return isOutdated(blocks) ? null : table
}

/** The forecast to draw: the table's own, or none while blocks are being chosen that it cannot hold for. */
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

/** The forecast entry of one player, or null when no attacker is aimed at them. */
export function playerOutlook(forecast: CombatForecast | null, seat: PlayerId): PlayerForecast | null {
  return forecast?.players.find((player) => player.player === seat) ?? null
}

const unsubscribed = (): (() => void) => () => undefined

// Listens to the pending store only for a caller the forecast mentions, so the many cards and panels it says
// nothing of are not redrawn when the first block is chosen.
function useOutdated(listen: boolean): boolean {
  const store = usePendingStore()
  return useSyncExternalStore(
    listen ? store.subscribe : unsubscribed,
    () => listen && isOutdated(store.get().blocks),
  )
}

/** What the drawn forecast says of one card (a "-N" badge, a doomed look); null for the many it says nothing of. */
export function useCreatureOutlook(id: ObjectId): CreatureOutlook | null {
  const entry = usePlay(useShallow((state) => creatureOutlook(state.view?.combat_forecast ?? null, id)))
  return useOutdated(entry !== null) ? null : entry
}

/** What the drawn forecast says of one seat's life, for its panel; null when nobody attacks that seat. */
export function usePlayerOutlook(seat: PlayerId): PlayerForecast | null {
  const entry = usePlay(useShallow((state) => playerOutlook(state.view?.combat_forecast ?? null, seat)))
  return useOutdated(entry !== null) ? null : entry
}
