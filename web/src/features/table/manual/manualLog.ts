/** The log lines of manual actions: the sentence of an action and the events of its change that the sentence already tells. */
import type { LogEntry } from '@/play/store'
import type { Event, EventView, ManualAction, ObjectId, Zone } from '@/protocol/generated'
import { describeHiddenManual, describeManual, joinWords } from '../eventText'
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
  LookTop: ['LibraryLooked'],
  ArrangeTop: ['Scried', 'Surveilled', 'ZoneChanged'],
  Shuffle: ['LibraryShuffled'],
  Reveal: ['Revealed'],
  Note: [],
  Roll: ['ManualRolled'],
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

/**
 * The cards the `count` events after the action at `at` tell the viewer: a look at the library, a reveal, or the cards
 * drawn (only a viewer who may see them gets a public draw; the others get a hidden one and so no cards).
 */
function shownIn(log: readonly LogEntry[], at: number, count: number): ObjectId[] | undefined {
  const drawn: ObjectId[] = []
  for (const entry of log.slice(at + 1, at + 1 + count)) {
    const inner = 'Public' in entry.event ? entry.event.Public : null
    if (inner === null || typeof inner === 'string') continue
    if ('LibraryLooked' in inner) return inner.LibraryLooked.cards
    if ('Revealed' in inner) return inner.Revealed.cards
    if ('CardDrawn' in inner) drawn.push(inner.CardDrawn.object)
  }
  return drawn.length === 0 ? undefined : drawn
}

/** The result the `count` events after the action at `at` tell: a coin or a die rolled by hand. */
function rolledIn(log: readonly LogEntry[], at: number, count: number): number | undefined {
  for (const entry of log.slice(at + 1, at + 1 + count)) {
    const inner = 'Public' in entry.event ? entry.event.Public : null
    if (inner !== null && typeof inner !== 'string' && 'ManualRolled' in inner)
      return inner.ManualRolled.result
  }
  return undefined
}

/** A manual action in the log: how many events after it tell the same thing and where a moved card came from. */
export interface ManualRun {
  /** The events right after the action that its sentence tells again. */
  consumed: number
  from: Zone | undefined
  /** The cards the run told the viewer: those looked at, revealed or drawn. */
  shown?: ObjectId[]
  /** The result of a coin or a die rolled by hand. */
  rolled?: number
  /** The names `LogEntry.names` kept for the action and the events its sentence tells again. */
  names?: Record<ObjectId, string>
}

/** For the event at `at`, the run of a manual action; null when it is not one. */
export function manualRun(log: readonly LogEntry[], at: number): ManualRun | null {
  const entry = log[at]
  if (entry === undefined) return null
  const event = 'Public' in entry.event ? entry.event.Public : null
  const hidden = 'Hidden' in entry.event ? entry.event.Hidden : null
  if (event !== null && typeof event !== 'string' && 'ManualAction' in event) {
    const consumed = changeLength(event, log, at)
    const shown = shownIn(log, at, consumed)
    const rolled = rolledIn(log, at, consumed)
    const kept = Object.assign({}, ...log.slice(at, at + 1 + consumed).map((each) => each.names))
    return {
      consumed,
      from: leftFrom(log, at, consumed),
      ...(shown === undefined ? {} : { shown }),
      ...(rolled === undefined ? {} : { rolled }),
      ...(Object.keys(kept).length === 0 ? {} : { names: kept }),
    }
  }
  // A move the viewer cannot name is told by its hidden twin, whose change is the hidden `ZoneChanged` after it.
  if (hidden !== null && typeof hidden === 'object' && 'ManualMove' in hidden) {
    const next = log[at + 1]
    const consumed = next !== undefined && nameOf(next.event) === 'ZoneChanged' ? 1 : 0
    return { consumed, from: leftFrom(log, at, consumed) }
  }
  return null
}

/** `names` with the card names a log entry remembered (`LogEntry.names`) read first. */
export function remember(
  names: EventNames,
  remembered: Readonly<Record<ObjectId, string>> | undefined,
): EventNames {
  if (remembered === undefined) return names
  return { ...names, card: (id) => remembered[id] ?? names.card(id) }
}

/** The longest clause the log quotes whole. */
const QUOTE_CHARS = 160

const quoted = (text: string): string =>
  `“${text.length > QUOTE_CHARS ? `${text.slice(0, QUOTE_CHARS - 1)}…` : text}”`

/** The card an event puts in front of the table, and whether it is cast and still to resolve; null for any other event. */
function momentOf(event: EventView): { id: ObjectId; cast: boolean } | null {
  if (!('Public' in event) || typeof event.Public === 'string') return null
  const { Public: shown } = event
  if ('SpellCast' in shown) return { id: shown.SpellCast.object, cast: true }
  if ('SpellResolved' in shown) return { id: shown.SpellResolved.object, cast: false }
  if ('LandPlayed' in shown) return { id: shown.LandPlayed.object, cast: false }
  if ('ZoneChanged' in shown) {
    const { object, to } = shown.ZoneChanged
    return to === 'Battlefield' ? { id: object, cast: false } : null
  }
  if ('ManualAction' in shown) {
    const { action } = shown.ManualAction
    return 'Move' in action && action.Move.to === 'Battlefield'
      ? { id: action.Move.object, cast: false }
      : null
  }
  return null
}

/** A triggered clause ("Whenever ...", "At the beginning of ...", possibly after an ability word such as "Landfall — "). */
const TRIGGERED = /^(?:[^—:.]{1,40} — )?(?:when|whenever|at the beginning of|at the end of|at end of)\b/i

/**
 * When the table applies each kind of clause of a card that is cast: an effect of a spell as it resolves, a triggered
 * clause whenever it triggers, any other clause of a permanent (static or activated) while it is on the battlefield.
 */
export function applyWhen(clauses: readonly string[], permanent: boolean): string {
  const moments = clauses.map((clause) => {
    if (TRIGGERED.test(clause.trim())) return 'whenever it triggers'
    return permanent ? 'while it is on the battlefield' : 'when it resolves'
  })
  return joinWords([...new Set(moments)])
}

/**
 * What a card leaves to the table when it is cast, resolves, is played or enters the battlefield (by an effect or by
 * hand): the clauses of its rules the engine does not run, quoted, so the seat sees what to apply by hand at the
 * moment it can. At the cast the line says when each kind applies (`applyWhen`); a permanent spell enters as it
 * resolves, so it is told there and not again. Null for any other event and for a card without such clauses (or one
 * whose clauses the viewer does not know).
 */
export function byHandLine(event: EventView, names: EventNames): string | null {
  const moment = momentOf(event)
  const clauses = moment === null ? [] : (names.clauses?.(moment.id) ?? [])
  if (moment === null || clauses.length === 0) return null
  const when = moment.cast ? ` ${applyWhen(clauses, names.permanent?.(moment.id) ?? false)}` : ''
  return `${names.card(moment.id)} has rules to apply by hand${when}: ${clauses.map(quoted).join('; ')}`
}

/** The sentence of a manual action (or of the hidden twin of a move) as the viewer reads it. */
export function manualText(
  event: EventView,
  from: Zone | undefined,
  names: EventNames,
  shown?: readonly ObjectId[],
  rolled?: number,
): string {
  if ('Public' in event && typeof event.Public !== 'string' && 'ManualAction' in event.Public) {
    const { player, action } = event.Public.ManualAction
    return describeManual(player, action, names, from, shown, rolled)
  }
  if ('Hidden' in event && typeof event.Hidden === 'object' && 'ManualMove' in event.Hidden) {
    return describeHiddenManual(event.Hidden.ManualMove.player, event.Hidden.ManualMove.to, names, from)
  }
  return ''
}
