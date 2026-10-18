/**
 * The decorations playing right now. `play` takes the effects of a batch and keys them by card, seat or battlefield
 * row; each expires on its own timer, and the timer only removes the entry, so the markup returns to what the game
 * says. The store never holds game state and nothing waits on it: it is read only by the parts that draw decoration.
 */
import { createStore } from 'zustand/vanilla'
import type { StoreApi } from 'zustand/vanilla'
import { prefersReducedMotion } from '@/hooks/useReducedMotion'
import { browserTimers } from '@/net/timers'
import type { Timers } from '@/net/timers'
import type { ObjectId, PlayerId } from '@/protocol/generated'
import { FX_MS, isLifeEffect, NO_ENTRIES, NO_GHOSTS, rowKey } from './model'
import type { Effect, FxEntry, FxKind, Ghost } from './model'

/** `off` plays nothing, `reduced` only the highlights that need no motion, `full` everything. */
export type FxMode = 'off' | 'reduced' | 'full'

/** The most effects one batch may start; a table-wide event at 12 seats stays cheap to draw. */
export const MAX_EFFECTS = 120

/** What is playing: entries per card and per seat, ghosts per battlefield row (`rowKey`). Replaced, never changed. */
export interface FxState {
  readonly objects: ReadonlyMap<ObjectId, readonly FxEntry[]>
  readonly seats: ReadonlyMap<PlayerId, readonly FxEntry[]>
  readonly ghosts: ReadonlyMap<string, readonly Ghost[]>
}

/** What `createFx` can be given; tests replace the clock and the mode. */
export interface FxOptions {
  /** The clock for expiry; the browser's by default. */
  timers?: Timers
  /** Read at every batch: how the decorations run now. By default `reduced` while the system asks for less motion. */
  mode?: () => FxMode
  /** Multiplies every duration, for looking at an effect slowly during development; 1 by default. */
  slow?: number
}

/** The decoration layer of one table. */
export interface Fx {
  /** The playing decorations; read through `useCardFx`, `useSeatFx` and `useGhosts`. */
  readonly store: StoreApi<FxState>
  /** The duration multiplier. */
  readonly slow: number
  /** Starts the effects of a batch, each expiring on its own after its time. Nothing at all while the mode is `off`. */
  play(effects: readonly Effect[]): void
  /** Ends every effect now and cancels the timers. */
  clear(): void
}

/** `reduced` while the person asks their system for less motion, else `full`. */
export function systemMode(): FxMode {
  return prefersReducedMotion() ? 'reduced' : 'full'
}

const emptyState = (): FxState => ({ objects: new Map(), seats: new Map(), ghosts: new Map() })

/** Kinds that read as a still highlight, so they stay under reduced motion. */
const isStill = (effect: Effect): boolean => effect.kind !== 'died' && effect.kind !== 'entered'

/** `list` with `entry` in place of any entry of the same kind. */
const put = (list: readonly FxEntry[] | undefined, entry: FxEntry): readonly FxEntry[] => [
  ...(list ?? NO_ENTRIES).filter((existing) => existing.kind !== entry.kind),
  entry,
]

/** `map` without the items whose token is in `tokens`; the very same map when none is. */
function without<K, T extends { readonly token: number }>(
  map: ReadonlyMap<K, readonly T[]>,
  tokens: ReadonlySet<number>,
): ReadonlyMap<K, readonly T[]> {
  let next: Map<K, readonly T[]> | null = null
  for (const [key, list] of map) {
    const kept = list.filter((item) => !tokens.has(item.token))
    if (kept.length === list.length) continue
    next ??= new Map(map)
    if (kept.length === 0) next.delete(key)
    else next.set(key, kept)
  }
  return next ?? map
}

/** A decoration layer with nothing playing. */
export function createFx(options: FxOptions = {}): Fx {
  const timers = options.timers ?? browserTimers
  const modeNow = options.mode ?? systemMode
  const slow = options.slow ?? 1
  const store = createStore<FxState>(emptyState)
  const waiting = new Set<number>()
  let last = 0

  // One timer per kind and batch: a board wipe ends in one update, not one per card.
  function expire(tokens: ReadonlySet<number>): void {
    const state = store.getState()
    const objects = without(state.objects, tokens)
    const seats = without(state.seats, tokens)
    const ghosts = without(state.ghosts, tokens)
    if (objects !== state.objects || seats !== state.seats || ghosts !== state.ghosts) {
      store.setState({ objects, seats, ghosts })
    }
  }

  function play(effects: readonly Effect[]): void {
    const mode = modeNow()
    if (mode === 'off') return
    const shown = (mode === 'reduced' ? effects.filter(isStill) : effects).slice(0, MAX_EFFECTS)
    if (shown.length === 0) return
    const state = store.getState()
    const objects = new Map(state.objects)
    const seats = new Map(state.seats)
    const ghosts = new Map(state.ghosts)
    const started = new Map<FxKind, Set<number>>()
    for (const effect of shown) {
      last += 1
      const token = last
      started.set(effect.kind, (started.get(effect.kind) ?? new Set<number>()).add(token))
      if (effect.kind === 'died') {
        const key = rowKey(effect.seat, effect.row)
        const ghost: Ghost = {
          token,
          object: effect.snapshot,
          before: effect.before,
          damage: effect.damage,
        }
        ghosts.set(key, [...(ghosts.get(key) ?? NO_GHOSTS), ghost])
      } else if (isLifeEffect(effect)) {
        const entry: FxEntry = { kind: effect.kind, token, amount: effect.amount, own: false }
        seats.set(effect.seat, put(seats.get(effect.seat), entry))
      } else {
        const entry: FxEntry = {
          kind: effect.kind,
          token,
          amount: 'amount' in effect ? effect.amount : 0,
          own: 'own' in effect ? effect.own : false,
        }
        objects.set(effect.object, put(objects.get(effect.object), entry))
      }
    }
    store.setState({ objects, seats, ghosts })
    for (const [kind, tokens] of started) {
      const timer = timers.setTimeout(() => {
        waiting.delete(timer)
        expire(tokens)
      }, FX_MS[kind] * slow)
      waiting.add(timer)
    }
  }

  function clear(): void {
    for (const timer of waiting) timers.clearTimeout(timer)
    waiting.clear()
    const { objects, seats, ghosts } = store.getState()
    if (objects.size + seats.size + ghosts.size > 0) store.setState(emptyState())
  }

  return { store, slow, play, clear }
}

/** The layer that cards outside a table share: it plays nothing. */
export const NO_FX: Fx = createFx({ mode: () => 'off' })
