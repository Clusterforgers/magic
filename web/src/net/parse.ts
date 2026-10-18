/**
 * The wire boundary: turns one WebSocket frame into a typed `ServerMessage` or a `FrameError`, never an exception.
 *
 * The envelope of every message and everything the client logic reads (ids, sequence numbers, the table and its
 * players, objects, event wrappers) is checked. Leaf values such as card faces and option lists are trusted: they
 * come from the generated bindings, which a Rust test keeps in step with the server.
 */
import type {
  AttackerForecast,
  AttackerView,
  CombatForecast,
  CreatureForecast,
  Decision,
  ErrorCode,
  EventView,
  ForecastDamage,
  GameResult,
  LobbySeat,
  ObjectView,
  PlayerForecast,
  PlayerView,
  ServerMessage,
  StateView,
  TableView,
  YieldUntil,
} from '@/protocol/generated'
import { isCount, isFlag, isInt, isListOf, isRecord, isText } from './check'

/** Frames longer than this many characters are dropped unread; the biggest real message is a snapshot of about 50 KB. */
export const MAX_FRAME_CHARS = 1_048_576

/** Why a frame was dropped. */
export type FrameErrorKind = 'not_text' | 'too_large' | 'not_json' | 'unknown_type' | 'bad_shape'

/** A frame that could not be used; `detail` is one line for logs and tests. */
export interface FrameError {
  kind: FrameErrorKind
  detail: string
}

/** The outcome of reading one frame. */
export type ParsedFrame = { ok: true; message: ServerMessage } | { ok: false; error: FrameError }

const MESSAGE_TYPES: Record<ServerMessage['type'], true> = {
  Welcome: true,
  Lobby: true,
  Decision: true,
  Forecast: true,
  Events: true,
  Snapshot: true,
  Chat: true,
  Error: true,
  GameOver: true,
  Yielding: true,
}

const ERROR_CODES: Record<ErrorCode, true> = {
  UnsupportedVersion: true,
  BadMessage: true,
  UnknownGame: true,
  StaleDecision: true,
  IllegalAnswer: true,
  Internal: true,
}

const YIELD_UNTIL: Record<YieldUntil, true> = { Resolve: true, EndOfTurn: true, MyTurn: true }

const isYieldUntil = (value: unknown): value is YieldUntil =>
  isText(value) && Object.hasOwn(YIELD_UNTIL, value)
// The server states the yield it holds as one of the three words, or null for none; an absent field is not none.
const isYielding = (value: unknown): value is YieldUntil | null => value === null || isYieldUntil(value)

const isMessageType = (value: string): value is ServerMessage['type'] => Object.hasOwn(MESSAGE_TYPES, value)
const isErrorCode = (value: unknown): value is ErrorCode => isText(value) && Object.hasOwn(ERROR_CODES, value)

const fail = (kind: FrameErrorKind, detail: string): ParsedFrame => ({ ok: false, error: { kind, detail } })

/** Reads one raw frame (`data` of a WebSocket message event) into a server message. */
export function parseFrame(data: unknown): ParsedFrame {
  if (typeof data !== 'string') return fail('not_text', 'binary frames are not part of the protocol')
  if (data.length > MAX_FRAME_CHARS) {
    return fail('too_large', `a frame of ${data.length} characters is over the limit of ${MAX_FRAME_CHARS}`)
  }
  let json: unknown
  try {
    json = JSON.parse(data)
  } catch {
    return fail('not_json', 'the frame is not valid JSON')
  }
  return parseMessage(json)
}

/** Checks an already decoded JSON value and returns it as a server message. */
export function parseMessage(value: unknown): ParsedFrame {
  if (!isRecord(value)) return fail('bad_shape', 'a message must be a JSON object')
  const type = value.type
  if (!isText(type)) return fail('bad_shape', 'a message needs a text "type"')
  if (!isMessageType(type))
    return fail('unknown_type', `unknown message type ${JSON.stringify(type.slice(0, 40))}`)
  const built = build(type, value)
  return typeof built === 'string' ? fail('bad_shape', `${type}: ${built}`) : { ok: true, message: built }
}

const isIdList = (value: unknown): value is number[] => isListOf(value, isCount)
// An externally tagged enum value: a bare name for a unit variant, a one-key object for the others.
const isTag = (value: unknown): boolean => isText(value) || isRecord(value)
const isOneKey = (value: Record<string, unknown>): boolean => Object.keys(value).length === 1

function isZone(value: unknown): boolean {
  return isRecord(value) && isCount(value.count) && (value.cards === null || isIdList(value.cards))
}

function isPlayerView(value: unknown): value is PlayerView {
  return (
    isRecord(value) &&
    isCount(value.seat) &&
    isInt(value.life) &&
    (value.lost === null || isTag(value.lost)) &&
    isRecord(value.mana) &&
    Array.isArray(value.mana.counts) &&
    value.mana.counts.length === 6 &&
    value.mana.counts.every(isCount) &&
    isCount(value.mulligans) &&
    isTag(value.mulligan) &&
    isZone(value.hand) &&
    isZone(value.library) &&
    isZone(value.graveyard) &&
    isZone(value.command)
  )
}

function isAttacker(value: unknown): value is AttackerView {
  return (
    isRecord(value) &&
    isCount(value.object) &&
    isTag(value.target) &&
    isFlag(value.blocked) &&
    isIdList(value.blockers)
  )
}

/** True for a game result: `"Draw"` or `{ Winner: seat }`. */
export function isGameResult(value: unknown): value is GameResult {
  return value === 'Draw' || (isRecord(value) && isOneKey(value) && isCount(value.Winner))
}

// A one-key object such as `{ Player: 1 }`: the key is one of `tags` and the value an id or seat.
function isTaggedId(value: unknown, tags: readonly string[]): boolean {
  if (!isRecord(value) || !isOneKey(value)) return false
  const [tag] = Object.keys(value)
  return tag !== undefined && tags.includes(tag) && isCount(value[tag])
}

function isForecastDamage(value: unknown): value is ForecastDamage {
  return (
    isRecord(value) &&
    isCount(value.source) &&
    isTaggedId(value.target, ['Player', 'Object']) &&
    isCount(value.amount)
  )
}

function isAttackerForecast(value: unknown): value is AttackerForecast {
  return (
    isRecord(value) &&
    isCount(value.attacker) &&
    isTaggedId(value.target, ['Player', 'Planeswalker']) &&
    isFlag(value.blocked) &&
    isIdList(value.blockers) &&
    isListOf(value.deals, isForecastDamage) &&
    isListOf(value.receives, isForecastDamage)
  )
}

function isCreatureForecast(value: unknown): value is CreatureForecast {
  return isRecord(value) && isCount(value.object) && isCount(value.damage) && isFlag(value.dies)
}

function isPlayerForecast(value: unknown): value is PlayerForecast {
  return (
    isRecord(value) &&
    isCount(value.player) &&
    isCount(value.damage) &&
    isInt(value.life) &&
    isInt(value.life_after) &&
    isFlag(value.lethal)
  )
}

/** True for the combat forecast of a table view: attackers, creatures and players, each checked down to its leaves. */
export function isCombatForecast(value: unknown): value is CombatForecast {
  return (
    isRecord(value) &&
    isListOf(value.attackers, isAttackerForecast) &&
    isListOf(value.creatures, isCreatureForecast) &&
    isListOf(value.players, isPlayerForecast)
  )
}

function isTableView(value: unknown): value is TableView {
  return (
    isRecord(value) &&
    isCount(value.viewer) &&
    isText(value.format) &&
    isText(value.stage) &&
    isCount(value.first_player) &&
    isRecord(value.turn) &&
    isCount(value.turn.number) &&
    isCount(value.turn.active) &&
    isText(value.turn.step) &&
    isCount(value.turn.lands_played) &&
    (value.priority === null ||
      (isRecord(value.priority) && isCount(value.priority.holder) && isIdList(value.priority.passed))) &&
    isListOf(value.players, isPlayerView) &&
    isZone(value.battlefield) &&
    isZone(value.stack) &&
    isZone(value.exile) &&
    isRecord(value.combat) &&
    isListOf(value.combat.attackers, isAttacker) &&
    isFlag(value.combat.attackers_declared) &&
    isIdList(value.combat.blockers_declared) &&
    (value.combat_forecast === null || isCombatForecast(value.combat_forecast)) &&
    (value.result === null || isGameResult(value.result))
  )
}

function isObjectView(value: unknown): value is ObjectView {
  return (
    isRecord(value) &&
    isCount(value.id) &&
    isCount(value.owner) &&
    isCount(value.controller) &&
    isTag(value.zone) &&
    isText(value.name) &&
    (value.mana_cost === null || Array.isArray(value.mana_cost)) &&
    isListOf(value.supertypes, isText) &&
    isListOf(value.types, isText) &&
    isListOf(value.subtypes, isText) &&
    isListOf(value.colors, isText) &&
    (value.power === null || isInt(value.power)) &&
    (value.toughness === null || isInt(value.toughness)) &&
    Array.isArray(value.abilities) &&
    (value.permanent === null ||
      (isRecord(value.permanent) &&
        isFlag(value.permanent.tapped) &&
        isCount(value.permanent.damage) &&
        isFlag(value.permanent.summoning_sick)))
  )
}

/** True for a whole `StateView` (the table plus its objects) as a snapshot or a fixture carries it. */
export function isStateView(value: unknown): value is StateView {
  return isRecord(value) && isListOf(value.objects, isObjectView) && isTableView(value)
}

function isLobbySeat(value: unknown): value is LobbySeat {
  return (
    isRecord(value) &&
    isCount(value.seat) &&
    isText(value.name) &&
    isFlag(value.connected) &&
    isFlag(value.ready)
  )
}

function isEventView(value: unknown): value is EventView {
  return isRecord(value) && isOneKey(value) && (isTag(value.Public) || isTag(value.Hidden))
}

function isDecision(value: unknown): value is Decision {
  return (
    isRecord(value) &&
    isCount(value.player) &&
    isRecord(value.kind) &&
    isOneKey(value.kind) &&
    Object.values(value.kind).every(isRecord)
  )
}

// A string is the name of the offending field, so the caller can say what was wrong.
type Built = ServerMessage | string

function build(type: ServerMessage['type'], v: Record<string, unknown>): Built {
  switch (type) {
    case 'Welcome':
      if (!isCount(v.protocol_version)) return '"protocol_version" must be a whole number'
      if (!isText(v.game)) return '"game" must be text'
      if (!isCount(v.player)) return '"player" must be a seat number'
      if (!isCount(v.seats)) return '"seats" must be a whole number'
      if (!isFlag(v.auto_pass)) return '"auto_pass" must be true or false'
      if (!isYielding(v.yielding)) return '"yielding" must be Resolve, EndOfTurn, MyTurn or null'
      return {
        type,
        protocol_version: v.protocol_version,
        game: v.game,
        player: v.player,
        seats: v.seats,
        auto_pass: v.auto_pass,
        yielding: v.yielding,
      }
    case 'Lobby':
      if (!isListOf(v.seats, isLobbySeat)) {
        return '"seats" must be a list of seats with a name, connected and ready'
      }
      // The lobby is drawn by seat number, so a roster that skips or repeats one is not one to draw.
      if (v.seats.some((seat, index) => seat.seat !== index)) return '"seats" must be listed in seat order'
      if (!isCount(v.needed)) return '"needed" must be a whole number'
      return { type, seats: v.seats, needed: v.needed }
    case 'Decision':
      if (!isCount(v.id)) return '"id" must be a whole number'
      if (!isDecision(v.decision)) return '"decision" must name a player and exactly one decision kind'
      // Absent means no clock; the server never sends null, so null is as wrong as text.
      if (v.deadline_ms !== undefined && !isCount(v.deadline_ms)) {
        return '"deadline_ms" must be a whole number of milliseconds'
      }
      return v.deadline_ms === undefined
        ? { type, id: v.id, decision: v.decision }
        : { type, id: v.id, decision: v.decision, deadline_ms: v.deadline_ms }
    case 'Forecast':
      if (!isCount(v.decision)) return '"decision" must be a whole number'
      if (v.forecast !== null && !isCombatForecast(v.forecast)) {
        return '"forecast" must be null or a combat forecast'
      }
      return { type, decision: v.decision, forecast: v.forecast }
    case 'Events':
      if (!isCount(v.from_seq)) return '"from_seq" must be a whole number'
      if (!isListOf(v.events, isEventView)) return '"events" must be a list of Public or Hidden events'
      if (!isTableView(v.table)) return '"table" is not a table view'
      if (!isListOf(v.objects, isObjectView)) return '"objects" must be a list of objects with ids'
      if (!isIdList(v.gone)) return '"gone" must be a list of ids'
      return {
        type,
        from_seq: v.from_seq,
        events: v.events,
        table: v.table,
        objects: v.objects,
        gone: v.gone,
      }
    case 'Snapshot':
      if (!isCount(v.seq)) return '"seq" must be a whole number'
      if (!isStateView(v.state)) return '"state" is not a state view'
      return { type, seq: v.seq, state: v.state }
    case 'Chat':
      if (!isCount(v.from)) return '"from" must be a seat number'
      if (!isText(v.text)) return '"text" must be text'
      return { type, from: v.from, text: v.text }
    case 'Error':
      if (!isErrorCode(v.code)) return '"code" is not a known error code'
      if (!isText(v.message)) return '"message" must be text'
      return { type, code: v.code, message: v.message }
    case 'GameOver':
      if (!isGameResult(v.result)) return '"result" must be Draw or a Winner'
      return { type, result: v.result }
    case 'Yielding':
      if (!isYielding(v.until)) return '"until" must be Resolve, EndOfTurn, MyTurn or null'
      return { type, until: v.until }
    default: {
      // Compile-time exhaustiveness: a new `ServerMessage` variant stops building here until it is handled.
      const unhandled: never = type
      return `no reader for message type ${String(unhandled)}`
    }
  }
}
