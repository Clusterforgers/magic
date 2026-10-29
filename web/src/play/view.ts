/**
 * The client fold: ports of `StateView::apply` and `StateView::from_first_batch` (crates/mtg-protocol/src/apply.rs).
 *
 * Everything here is pure and never touches its inputs. Whatever did not change keeps its identity (an object, a
 * player, a zone, the whole view), so a React tree built on these values re-renders only what moved.
 */
import { isRecord } from '@/net/check'
import type { EventBatch, ObjectId, ObjectView, PlayerView, StateView, TableView } from '@/protocol/generated'

/** Deep equality for the JSON values a view is made of. */
export function sameJson(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true
  if (Array.isArray(a))
    return Array.isArray(b) && a.length === b.length && a.every((item, i) => sameJson(item, b[i]))
  if (isRecord(a) && isRecord(b)) {
    const keys = Object.keys(a)
    return (
      keys.length === Object.keys(b).length &&
      keys.every((key) => Object.hasOwn(b, key) && sameJson(a[key], b[key]))
    )
  }
  return false
}

const keep = <T>(previous: T, next: T): T => (sameJson(previous, next) ? previous : next)

const sameItems = <T>(a: readonly T[], b: readonly T[]): boolean =>
  a.length === b.length && a.every((item, i) => item === b[i])

function shallowSame(a: Record<string, unknown>, b: Record<string, unknown>): boolean {
  const keys = Object.keys(b)
  return keys.length === Object.keys(a).length && keys.every((key) => Object.is(a[key], b[key]))
}

// The field lists below are spelled out on purpose: a field added to a generated type stops compiling here
// until it is shared too, instead of silently losing its identity.
function sharePlayer(previous: PlayerView, next: PlayerView): PlayerView {
  if (sameJson(previous, next)) return previous
  return {
    seat: next.seat,
    life: next.life,
    lost: keep(previous.lost, next.lost),
    mana: keep(previous.mana, next.mana),
    mulligans: next.mulligans,
    mulligan: keep(previous.mulligan, next.mulligan),
    hand: keep(previous.hand, next.hand),
    library: keep(previous.library, next.library),
    graveyard: keep(previous.graveyard, next.graveyard),
    command: keep(previous.command, next.command),
    bank: next.bank,
  }
}

function sharePlayers(previous: PlayerView[], next: PlayerView[]): PlayerView[] {
  const shared = next.map((player, index) => {
    const before = previous[index]
    return before === undefined ? player : sharePlayer(before, player)
  })
  return sameItems(previous, shared) ? previous : shared
}

function shareTable(previous: TableView, next: TableView): TableView {
  return {
    viewer: next.viewer,
    format: next.format,
    stage: next.stage,
    first_player: next.first_player,
    turn: keep(previous.turn, next.turn),
    priority: keep(previous.priority, next.priority),
    players: sharePlayers(previous.players, next.players),
    battlefield: keep(previous.battlefield, next.battlefield),
    stack: keep(previous.stack, next.stack),
    exile: keep(previous.exile, next.exile),
    combat: keep(previous.combat, next.combat),
    combat_forecast: keep(previous.combat_forecast, next.combat_forecast),
    result: keep(previous.result, next.result),
  }
}

function shareObject(previous: ObjectView, next: ObjectView): ObjectView {
  if (sameJson(previous, next)) return previous
  // Every field of `next`, so an optional one (keywords, identity, activated) is never dropped.
  return {
    ...next,
    zone: keep(previous.zone, next.zone),
    mana_cost: keep(previous.mana_cost, next.mana_cost),
    supertypes: keep(previous.supertypes, next.supertypes),
    types: keep(previous.types, next.types),
    subtypes: keep(previous.subtypes, next.subtypes),
    colors: keep(previous.colors, next.colors),
    power: next.power,
    toughness: next.toughness,
    abilities: keep(previous.abilities, next.abilities),
    permanent: keep(previous.permanent, next.permanent),
  }
}

const byId = (a: ObjectView, b: ObjectView): number => a.id - b.id

function isSorted(objects: readonly ObjectView[]): boolean {
  return objects.every((object, i) => i === 0 || (objects[i - 1]?.id ?? -1) < object.id)
}

// Upserts, then removals (so an id in both is gone, as in the Rust fold), in id order.
function mergeObjects(previous: ObjectView[], upserts: ObjectView[], gone: ObjectId[]): ObjectView[] {
  if (upserts.length === 0 && gone.length === 0 && isSorted(previous)) return previous
  const merged = new Map<ObjectId, ObjectView>(
    previous.map((object): [ObjectId, ObjectView] => [object.id, object]),
  )
  for (const incoming of upserts) {
    const before = merged.get(incoming.id)
    merged.set(incoming.id, before === undefined ? incoming : shareObject(before, incoming))
  }
  for (const id of gone) merged.delete(id)
  const objects = [...merged.values()].sort(byId)
  return sameItems(previous, objects) ? previous : objects
}

function rebuild(previous: StateView, table: TableView, objects: ObjectView[]): StateView {
  const next: StateView = { ...shareTable(previous, table), objects }
  return shallowSame(previous, next) ? previous : next
}

/** Advances `view` by one batch: replaces the table, upserts `objects`, drops `gone`, keeps objects in id order. */
export function applyBatch(view: StateView, batch: EventBatch): StateView {
  return rebuild(view, batch.table, mergeObjects(view.objects, batch.objects, batch.gone))
}

/** The view a client with no snapshot builds from the batch at `from_seq` 0, or null for any later batch. */
export function fromFirstBatch(batch: EventBatch): StateView | null {
  if (batch.from_seq !== 0) return null
  return { ...batch.table, objects: mergeObjects([], batch.objects, batch.gone) }
}

/** The view a `Snapshot` sets, sharing whatever is unchanged with the view it replaces so a resync does not redraw the board. */
export function fromSnapshot(previous: StateView | null, snapshot: StateView): StateView {
  if (previous === null)
    return isSorted(snapshot.objects) ? snapshot : { ...snapshot, objects: [...snapshot.objects].sort(byId) }
  const before = new Map(previous.objects.map((object): [ObjectId, ObjectView] => [object.id, object]))
  const shared = snapshot.objects.map((object) => {
    const old = before.get(object.id)
    return old === undefined ? object : shareObject(old, object)
  })
  const ordered = isSorted(shared) ? shared : shared.sort(byId)
  return rebuild(previous, snapshot, sameItems(previous.objects, ordered) ? previous.objects : ordered)
}

/** The objects of a view keyed by id, for lookups from ids found in zones, the stack and decisions. */
export function indexById(objects: readonly ObjectView[]): Readonly<Record<ObjectId, ObjectView>> {
  return Object.fromEntries(objects.map((object): [ObjectId, ObjectView] => [object.id, object]))
}

/** The view with the bank of clock extensions of `seat` set; the server tells a spent extension with no events. */
export function withBank(view: StateView, seat: number, bank: number): StateView {
  if (view.players.every((player) => player.seat !== seat || player.bank === bank)) return view
  return {
    ...view,
    players: view.players.map((player) => (player.seat === seat ? { ...player, bank } : player)),
  }
}
