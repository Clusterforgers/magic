/** The log lines of manual actions: the sentence of an action and the events of its change that the sentence already tells. */
import type { LogEntry } from '@/play/store'
import type { Event, EventView, ManualAction, Zone } from '@/protocol/generated'
import { describeHiddenManual, describeManual } from '../eventText'
import type { EventNames } from '../eventText'

/** The events that carry out `action`, by variant name; `Move` also reads the zone it left from the first of them. */
const CHANGES: Record<ActionKind, readonly string[]> = {
  Move: ['ZoneChanged', 'LibraryBottomed', 'CardsBottomed', 'CountersChanged'],
  MoveTop: ['ZoneChanged', 'LibraryBottomed', 'CardsBottomed'],
  Counters: ['CountersChanged'],
  PlayerCounters: ['CountersChanged'],
  Life: ['LifeGained', 'LifeLost'],
  CreateToken: ['TokenCreated'],
  Tap: ['PermanentTapped'],
  Untap: ['PermanentUntapped'],
  Draw: ['CardDrawn'],
  Discard: ['CardDiscarded', 'ZoneChanged'],
  Attach: ['Attached'],
  Unattach: ['Unattached'],
  Damage: ['DamageDealt'],
  AddMana: ['ManaAdded'],
  Pump: ['EffectCreated'],
}

type ActionKind = ManualAction extends infer U ? (U extends unknown ? keyof U : never) : never

/** The name of an event of the public or hidden log, or null for a bare string variant. */
function nameOf(event: EventView): string | null {
  const inner = 'Public' in event ? event.Public : event.Hidden
  return typeof inner === 'string' ? inner : (Object.keys(inner)[0] ?? null)
}

/** The kind of action a public `ManualAction` event is, or null for any other event. */
export function manualKind(event: EventView): ActionKind | null {
  if (!('Public' in event) || typeof event.Public === 'string' || !('ManualAction' in event.Public))
    return null
  return Object.keys(event.Public.ManualAction.action)[0] as ActionKind
}

/** How many events after an action tell its change: one for most, one for each card or token of a count. */
function changeLength(event: Event, log: readonly LogEntry[], at: number): number {
  const kind = manualKind({ Public: event })
  if (kind === null) return 0
  const names = CHANGES[kind]
  let end = at + 1
  while (end < log.length) {
    const entry = log[end]
    const name = entry === undefined ? null : nameOf(entry.event)
    if (name === null || !names.includes(name)) break
    end += 1
  }
  return end - at - 1
}

/** The zone a card left in the events of a move by hand, if the viewer was told it. */
export function leftFrom(log: readonly LogEntry[], at: number, count: number): Zone | undefined {
  for (const entry of log.slice(at + 1, at + 1 + count)) {
    const inner = 'Public' in entry.event ? entry.event.Public : entry.event.Hidden
    if (typeof inner === 'object' && 'ZoneChanged' in inner) return inner.ZoneChanged.from
  }
  return undefined
}

/** A manual action in the log: how many events after it tell the same thing and where a moved card came from. */
export interface ManualRun {
  /** The events right after the action that its sentence tells again. */
  consumed: number
  from: Zone | undefined
}

/** For the event at `at`, the run of a manual action; null when it is not one. */
export function manualRun(log: readonly LogEntry[], at: number): ManualRun | null {
  const entry = log[at]
  if (entry === undefined) return null
  const event = 'Public' in entry.event ? entry.event.Public : null
  const hidden = 'Hidden' in entry.event ? entry.event.Hidden : null
  if (event !== null && typeof event !== 'string' && 'ManualAction' in event) {
    const consumed = changeLength(event, log, at)
    return { consumed, from: leftFrom(log, at, consumed) }
  }
  // A move the viewer cannot name is told by its hidden twin, whose change is the hidden `ZoneChanged` after it.
  if (hidden !== null && typeof hidden === 'object' && 'ManualMove' in hidden) {
    const next = log[at + 1]
    const consumed = next !== undefined && nameOf(next.event) === 'ZoneChanged' ? 1 : 0
    return { consumed, from: leftFrom(log, at, consumed) }
  }
  return null
}

/** The longest clause the log quotes whole. */
const QUOTE_CHARS = 160

const quoted = (text: string): string =>
  `“${text.length > QUOTE_CHARS ? `${text.slice(0, QUOTE_CHARS - 1)}…` : text}”`

/**
 * What a resolved spell or a played land leaves to the table: the clauses of its rules the engine does not run,
 * quoted, so the seat sees what to apply by hand at the moment it can. Null for any other event and for a card
 * without such clauses (or one whose clauses the viewer does not know).
 */
export function byHandLine(event: EventView, names: EventNames): string | null {
  if (!('Public' in event) || typeof event.Public === 'string') return null
  const { Public: shown } = event
  const id =
    'SpellResolved' in shown
      ? shown.SpellResolved.object
      : 'LandPlayed' in shown
        ? shown.LandPlayed.object
        : null
  const clauses = id === null ? [] : (names.clauses?.(id) ?? [])
  if (id === null || clauses.length === 0) return null
  return `${names.card(id)} has rules to apply by hand: ${clauses.map(quoted).join('; ')}`
}

/** The sentence of a manual action (or of the hidden twin of a move) as the viewer reads it. */
export function manualText(event: EventView, from: Zone | undefined, names: EventNames): string {
  if ('Public' in event && typeof event.Public !== 'string' && 'ManualAction' in event.Public) {
    const { player, action } = event.Public.ManualAction
    return describeManual(player, action, names, from)
  }
  if ('Hidden' in event && typeof event.Hidden === 'object' && 'ManualMove' in event.Hidden) {
    return describeHiddenManual(event.Hidden.ManualMove.player, event.Hidden.ManualMove.to, names, from)
  }
  return ''
}
