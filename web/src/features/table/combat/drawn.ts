/**
 * The forecast on screen, per card and per seat. One component (`CombatProviders`) publishes the forecast the seam
 * (`useBlockPreview`) says to draw, and each card or panel subscribes to its own entry only, the way cards read their
 * marks in `prompt/marks.ts`: a batch, or a block being chosen, redraws just the cards whose outlook changed, and when
 * the seam starts to return the engine's preview of a pending answer, nothing here needs to change.
 */
import { createContext, useCallback, useContext, useSyncExternalStore } from 'react'
import type { CombatForecast, ObjectId, PlayerForecast, PlayerId } from '@/protocol/generated'
import { creatureOutlook } from './forecast'
import type { CreatureOutlook } from './forecast'

/** The forecast now drawn, entry by entry. */
export interface DrawnForecastStore {
  /** Replaces the forecast drawn; only the listeners of an entry that changed are told. */
  publish(forecast: CombatForecast | null): void
  /** What the forecast says will happen to one creature, or null when there is nothing to show. */
  creature(id: ObjectId): CreatureOutlook | null
  /** What the forecast says will happen to one seat's life, or null when nobody attacks it. */
  player(seat: PlayerId): PlayerForecast | null
  /** Calls `listener` when the entry of creature `id` changes; returns the way to stop. */
  subscribeCreature(id: ObjectId, listener: () => void): () => void
  /** Calls `listener` when the entry of `seat` changes; returns the way to stop. */
  subscribePlayer(seat: PlayerId, listener: () => void): () => void
}

const sameOutlook = (a: CreatureOutlook, b: CreatureOutlook): boolean =>
  a.damage === b.damage && a.dies === b.dies

const samePlayer = (a: PlayerForecast, b: PlayerForecast): boolean =>
  a.damage === b.damage && a.life === b.life && a.life_after === b.life_after && a.lethal === b.lethal

type Listeners<K> = Map<K, Set<() => void>>

// The entries of `next`, keeping the very object of an entry that did not change, and the keys that did change.
function replace<K, V>(
  before: ReadonlyMap<K, V>,
  next: ReadonlyMap<K, V>,
  same: (a: V, b: V) => boolean,
): { kept: Map<K, V>; changed: K[] } {
  const kept = new Map<K, V>()
  const changed: K[] = []
  for (const [key, value] of next) {
    const old = before.get(key)
    if (old !== undefined && same(old, value)) kept.set(key, old)
    else {
      kept.set(key, value)
      changed.push(key)
    }
  }
  for (const key of before.keys()) if (!next.has(key)) changed.push(key)
  return { kept, changed }
}

function tell<K>(listeners: Listeners<K>, keys: readonly K[]): void {
  for (const key of keys) for (const listener of [...(listeners.get(key) ?? [])]) listener()
}

function listen<K>(listeners: Listeners<K>, key: K, listener: () => void): () => void {
  let set = listeners.get(key)
  if (set === undefined) {
    set = new Set()
    listeners.set(key, set)
  }
  set.add(listener)
  return () => {
    set.delete(listener)
    if (set.size === 0) listeners.delete(key)
  }
}

/** A store that draws no forecast. */
export function createDrawnForecastStore(): DrawnForecastStore {
  let creatures = new Map<ObjectId, CreatureOutlook>()
  let players = new Map<PlayerId, PlayerForecast>()
  const creatureListeners: Listeners<ObjectId> = new Map()
  const playerListeners: Listeners<PlayerId> = new Map()
  return {
    publish(forecast) {
      const nextCreatures = new Map<ObjectId, CreatureOutlook>()
      for (const { object } of forecast?.creatures ?? []) {
        const outlook = creatureOutlook(forecast, object)
        if (outlook !== null) nextCreatures.set(object, outlook)
      }
      const nextPlayers = new Map<PlayerId, PlayerForecast>(
        (forecast?.players ?? []).map((player) => [player.player, player]),
      )
      const hurt = replace(creatures, nextCreatures, sameOutlook)
      const hit = replace(players, nextPlayers, samePlayer)
      creatures = hurt.kept
      players = hit.kept
      tell(creatureListeners, hurt.changed)
      tell(playerListeners, hit.changed)
    },
    creature: (id) => creatures.get(id) ?? null,
    player: (seat) => players.get(seat) ?? null,
    subscribeCreature: (id, listener) => listen(creatureListeners, id, listener),
    subscribePlayer: (seat, listener) => listen(playerListeners, seat, listener),
  }
}

/** Cards and panels outside a table share one store that nothing publishes to, so they draw no forecast. */
export const DrawnForecastContext = createContext<DrawnForecastStore>(createDrawnForecastStore())

/** What the drawn forecast says of one card (a "-N" badge, a doomed look); null for the many it says nothing of. */
export function useCreatureOutlook(id: ObjectId): CreatureOutlook | null {
  const store = useContext(DrawnForecastContext)
  const subscribe = useCallback((listener: () => void) => store.subscribeCreature(id, listener), [store, id])
  return useSyncExternalStore(subscribe, () => store.creature(id))
}

/** What the drawn forecast says of one seat's life, for its panel; null when nobody attacks that seat. */
export function usePlayerOutlook(seat: PlayerId): PlayerForecast | null {
  const store = useContext(DrawnForecastContext)
  const subscribe = useCallback(
    (listener: () => void) => store.subscribePlayer(seat, listener),
    [store, seat],
  )
  return useSyncExternalStore(subscribe, () => store.player(seat))
}
