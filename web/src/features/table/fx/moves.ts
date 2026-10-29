/**
 * The moves of a batch: which cards fly from one place to another and which marks they leave (docs/ui-design.md,
 * Playing is legible). Pure: it reads the game before and after the batch and the events in their order, never the
 * DOM or a clock. The travel layer measures the places; this only names them.
 */
import type { EventView, ObjectId, ObjectView, PlayerId, Target, Zone } from '@/protocol/generated'
import type { Moment } from './derive'

/** The motion tokens of moves, in milliseconds; `fx.css` repeats them as `--fx-<name>-ms`. */
export const MOVE_MS = {
  travel: 380,
  spotlight: 900,
  stagger: 120,
  arrow: 250,
  mark: 600,
} as const

/** The most moves one batch flies; the rest leave their marks only. */
export const MAX_MOVES = 8

/** What a move shows. */
export type MoveKind =
  'cast' | 'activated' | 'triggered' | 'resolved' | 'countered' | 'fizzled' | 'landed' | 'drawn' | 'moved'

/** A zone a seat's pod, tile or the hand dock draws as a chip (`data-fx-anchor="<seat>:<zone>"`). */
export type AnchorZone = 'hand' | 'library' | 'graveyard' | 'exile' | 'command' | 'battlefield'

/**
 * Where a move starts or ends: the card itself when the table draws it (`object`), else the anchor of its zone, which
 * the travel layer falls back to when the card is not in view. `stack` is the shared strip's stack.
 */
export type FxPlace =
  | { readonly stack: true; readonly object?: ObjectId }
  | { readonly seat: PlayerId; readonly zone: AnchorZone; readonly object?: ObjectId }

/** One card or ability flying from one place to another. */
export interface Move {
  readonly kind: MoveKind
  /** The object that flies; null for a card the viewer may not identify. */
  readonly object: ObjectId | null
  /** How to draw it in flight; null draws a card back. */
  readonly look: ObjectView | null
  readonly from: FxPlace
  readonly to: FxPlace
  /** Its place in the batch's sequence; it starts `order * MOVE_MS.stagger` after the first. */
  readonly order: number
  /** What a spell or ability on its way to the stack targets. */
  readonly targets: readonly Target[]
  /** It stands large over the shared strip before it lands. */
  readonly spotlight: boolean
}

/** What a mark says on a card. */
export type MarkKind =
  | 'activated'
  | 'triggered'
  | 'targeted'
  | 'resolved'
  | 'countered'
  | 'fizzled'
  | 'exiled'
  | 'bounced'
  | 'drawn'

/** A still mark on a card; `by` is the spell or ability that caused it, for its short arrow. */
export interface MoveMark {
  readonly kind: MarkKind
  readonly object: ObjectId
  readonly by?: ObjectId
}

/** The moves and marks of one batch, in event order. */
export interface Moves {
  readonly moves: readonly Move[]
  readonly marks: readonly MoveMark[]
}

/** No moves and no marks. */
export const NO_MOVES: Moves = { moves: [], marks: [] }

/** The place of a zone; a shared zone (exile, the battlefield) belongs to `seat`. */
export function placeOf(zone: Zone, seat: PlayerId, object?: ObjectId): FxPlace {
  const at = object === undefined ? {} : { object }
  if (zone === 'Stack') return { stack: true, ...at }
  if (zone === 'Battlefield' || zone === 'PhasedOut') return { seat, zone: 'battlefield', ...at }
  if (zone === 'Exile') return { seat, zone: 'exile', ...at }
  if ('Hand' in zone) return { seat: zone.Hand, zone: 'hand', ...at }
  if ('Library' in zone) return { seat: zone.Library, zone: 'library', ...at }
  if ('Graveyard' in zone) return { seat: zone.Graveyard, zone: 'graveyard', ...at }
  return { seat: zone.Command, zone: 'command', ...at }
}

/** The `data-fx-anchor` value of a place's zone. */
export function anchorOf(place: FxPlace): string {
  return 'stack' in place ? 'stack' : `${place.seat}:${place.zone}`
}

const flat = (targets: readonly (readonly Target[])[] | undefined): Target[] => targets?.flat() ?? []

/**
 * The moves and marks of one batch. A spell flies from where it was (the caster's hand when the viewer could not see
 * it) to the stack, and from the stack to where it ends when it resolves, is countered or fizzles; an ability flies
 * from its source to the stack; a land, a drawn card and a card a resolution moves fly between their zones. A spell
 * cast by another seat, or cast and resolved in the same batch, stands in the spotlight on its way to the stack.
 */
export function deriveMoves(before: Moment, after: Moment, events: readonly EventView[]): Moves {
  const { viewer } = after.view
  const moves: Omit<Move, 'order'>[] = []
  const marks: MoveMark[] = []
  // Where each object is as the events go by, starting from the moment before.
  const zones = new Map<ObjectId, Zone>()
  const zoneOf = (id: ObjectId): Zone | undefined => zones.get(id) ?? before.objectsById[id]?.zone
  const lookOf = (id: ObjectId): ObjectView | null => after.objectsById[id] ?? before.objectsById[id] ?? null
  const seatOf = (id: ObjectId, fallback: PlayerId): PlayerId => lookOf(id)?.controller ?? fallback
  const resolving: ObjectId[] = []
  const by = (): { by?: ObjectId } => {
    const top = resolving.at(-1)
    return top === undefined ? {} : { by: top }
  }
  const resolvesHere = new Set<ObjectId>()
  for (const view of events) {
    if ('Public' in view && typeof view.Public !== 'string' && 'ResolutionEnded' in view.Public)
      resolvesHere.add(view.Public.ResolutionEnded.object)
  }
  const target = (targets: Target[], cause: ObjectId): void => {
    for (const aimed of targets)
      if ('Object' in aimed) marks.push({ kind: 'targeted', object: aimed.Object.id, by: cause })
  }

  for (const view of events) {
    if ('Hidden' in view) {
      const hidden = view.Hidden
      if (typeof hidden !== 'string' && 'CardDrawn' in hidden) {
        const seat = hidden.CardDrawn.player
        moves.push({
          kind: 'drawn',
          object: null,
          look: null,
          from: { seat, zone: 'library' },
          to: { seat, zone: 'hand' },
          targets: [],
          spotlight: false,
        })
      } else if (typeof hidden !== 'string' && 'AbilityPutOnStack' in hidden) {
        const { object, controller } = hidden.AbilityPutOnStack
        moves.push({
          kind: 'activated',
          object,
          look: lookOf(object),
          from: { seat: controller, zone: 'hand' },
          to: { stack: true, object },
          targets: flat(after.objectsById[object]?.targets),
          spotlight: false,
        })
        zones.set(object, 'Stack')
      }
      continue
    }
    const event = view.Public
    if (typeof event === 'string') continue
    if ('SpellCast' in event) {
      const { player, object, choices } = event.SpellCast
      const targets = after.objectsById[object]?.targets ?? choices?.targets
      moves.push({
        kind: 'cast',
        object,
        look: lookOf(object),
        from: placeOf(zoneOf(object) ?? { Hand: player }, player, object),
        to: { stack: true, object },
        targets: flat(targets),
        spotlight: player !== viewer || resolvesHere.has(object),
      })
      target(flat(targets), object)
      zones.set(object, 'Stack')
    } else if ('LandPlayed' in event) {
      const { player, object } = event.LandPlayed
      moves.push({
        kind: 'landed',
        object,
        look: lookOf(object),
        from: placeOf(zoneOf(object) ?? { Hand: player }, player, object),
        to: { seat: player, zone: 'battlefield', object },
        targets: [],
        spotlight: false,
      })
      zones.set(object, 'Battlefield')
    } else if ('AbilityPutOnStack' in event) {
      const { object, source, snapshot, cause, choices } = event.AbilityPutOnStack
      const kind = cause === undefined ? 'activated' : 'triggered'
      const targets = after.objectsById[object]?.targets ?? choices?.targets
      moves.push({
        kind,
        object,
        look: lookOf(object) ?? lookOf(source.id),
        from: placeOf(snapshot.zone, snapshot.controller, source.id),
        to: { stack: true, object },
        targets: flat(targets),
        spotlight: false,
      })
      marks.push({ kind, object: source.id })
      target(flat(targets), object)
      zones.set(object, 'Stack')
    } else if ('ResolutionBegan' in event) {
      resolving.push(event.ResolutionBegan.object)
    } else if ('ResolutionEnded' in event) {
      const { object, outcome } = event.ResolutionEnded
      const index = resolving.lastIndexOf(object)
      if (index >= 0) resolving.splice(index, 1)
      const kind = outcome === 'Resolved' ? 'resolved' : outcome === 'Countered' ? 'countered' : 'fizzled'
      marks.push({ kind, object, ...(kind === 'countered' ? by() : {}) })
      const look = lookOf(object)
      // An ability ends on the stack; a spell goes on to where the resolution left it.
      if (look === null || look.kind === 'Ability') continue
      const end = after.objectsById[object]?.zone ?? { Graveyard: look.owner }
      if (end === 'Stack') continue
      moves.push({
        kind,
        object,
        look,
        from: { stack: true, object },
        to: placeOf(end, seatOf(object, look.owner), object),
        targets: [],
        spotlight: false,
      })
      zones.set(object, end)
    } else if ('CardDrawn' in event) {
      const { player, object } = event.CardDrawn
      moves.push({
        kind: 'drawn',
        object,
        look: lookOf(object),
        from: { seat: player, zone: 'library' },
        to: { seat: player, zone: 'hand', object },
        targets: [],
        spotlight: false,
      })
      if (player === viewer) marks.push({ kind: 'drawn', object })
      zones.set(object, { Hand: player })
    } else if ('ZoneChanged' in event) {
      const { object, from, to } = event.ZoneChanged
      // The stack's own moves are those of the resolution above.
      if (from === 'Stack' || to === 'Stack') continue
      const look = lookOf(object)
      const seat = look?.controller ?? look?.owner ?? viewer
      if (from === 'Battlefield') {
        if (to === 'Exile') marks.push({ kind: 'exiled', object, ...by() })
        else if (typeof to !== 'string' && 'Hand' in to) marks.push({ kind: 'bounced', object, ...by() })
      }
      moves.push({
        kind: 'moved',
        object,
        look,
        from: placeOf(from, seat, object),
        to: placeOf(to, seat, object),
        targets: [],
        spotlight: false,
      })
      zones.set(object, to)
    }
  }
  return {
    moves: moves.slice(0, MAX_MOVES).map((move, order) => ({ ...move, order })),
    marks,
  }
}
