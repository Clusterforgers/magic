/**
 * Table feedback: the words the pure derivation (`derive.ts`), the store (`store.ts`) and the components that draw
 * the decorations share. Nothing here touches React or the DOM.
 */
import type { ObjectId, ObjectView, PlayerId } from '@/protocol/generated'
import type { RowKind } from '../cardModel'

/** What happened, as a decoration tells it. */
export type FxKind = 'attacked' | 'blocked' | 'damaged' | 'healed' | 'died' | 'entered'

/**
 * How long each decoration stays on, in milliseconds (times the dev slow-down). The CSS keeps the same numbers as
 * `--fx-<kind>-ms` in `fx.css`, and a test holds the two equal; every one is between 250 and 600.
 */
export const FX_MS: Readonly<Record<FxKind, number>> = {
  entered: 350,
  attacked: 500,
  blocked: 500,
  damaged: 550,
  healed: 550,
  died: 500,
}

/** A creature joined an attack or a block and leans toward the other side; `own` is true for the viewer's creature. */
export interface LeanEffect {
  kind: 'attacked' | 'blocked'
  object: ObjectId
  own: boolean
}

/** A permanent came onto the battlefield. */
export interface EnterEffect {
  kind: 'entered'
  object: ObjectId
}

/** Damage was marked on a permanent that is still on the battlefield. */
export interface HurtEffect {
  kind: 'damaged'
  object: ObjectId
  amount: number
}

/** A seat lost (`damaged`) or gained (`healed`) `amount` life. */
export interface LifeEffect {
  kind: 'damaged' | 'healed'
  seat: PlayerId
  amount: number
}

/**
 * A permanent went from the battlefield to a graveyard. `snapshot` is how the table last drew it, `seat` and `row`
 * say where, and `before` is the card that stood after it in that row and still does (null: the end of the row), so
 * its ghost can stand in the old place for a moment. `damage` is the damage dealt to it in the same batch.
 */
export interface DiedEffect {
  kind: 'died'
  object: ObjectId
  seat: PlayerId
  row: RowKind
  snapshot: ObjectView
  before: ObjectId | null
  damage: number
}

/** One transient visual effect that a batch of events caused. */
export type Effect = LeanEffect | EnterEffect | HurtEffect | LifeEffect | DiedEffect

/** An effect on a seat's life total. */
export function isLifeEffect(effect: Effect): effect is LifeEffect {
  return 'seat' in effect && effect.kind !== 'died'
}

/** An effect that plays on a card on the battlefield. */
export function isCardEffect(effect: Effect): effect is LeanEffect | EnterEffect | HurtEffect {
  return effect.kind !== 'died' && 'object' in effect
}

/** One effect while it plays on a card or a life total. `token` is new for every effect, so a repeat restarts. */
export interface FxEntry {
  readonly kind: FxKind
  readonly token: number
  /** The number to float for `damaged` and `healed`; 0 for the kinds that have none. */
  readonly amount: number
  /** The creature is the viewer's, so it leans up the screen; false leans down. */
  readonly own: boolean
}

/** A permanent that died, drawn in its old place until its effect ends. */
export interface Ghost {
  readonly token: number
  readonly object: ObjectView
  readonly before: ObjectId | null
  readonly damage: number
}

/** No entries; one value, so a card with nothing playing keeps the same snapshot. */
export const NO_ENTRIES: readonly FxEntry[] = []

/** No ghosts. */
export const NO_GHOSTS: readonly Ghost[] = []

/** The `data-fx` value of a card or life total: the kinds playing now, space separated, or undefined for none. */
export function fxAttr(entries: readonly FxEntry[]): string | undefined {
  if (entries.length === 0) return undefined
  return entries.map((entry) => entry.kind).join(' ')
}

/** Which way a card leans (`data-fx-side`), or undefined while it does not lean. */
export function fxSide(entries: readonly FxEntry[]): 'own' | 'other' | undefined {
  const leaning = entries.find((entry) => entry.kind === 'attacked' || entry.kind === 'blocked')
  if (leaning === undefined) return undefined
  return leaning.own ? 'own' : 'other'
}

/** The key of the row of a seat's battlefield that a ghost belongs to. */
export function rowKey(seat: PlayerId, row: RowKind): string {
  return `${seat}:${row}`
}
