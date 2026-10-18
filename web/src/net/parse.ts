/**
 * The wire boundary: turns one WebSocket frame into a typed `ServerMessage` or a `FrameError`, never an exception.
 *
 * The envelope of every message and everything the client logic reads (ids, sequence numbers, the table and its
 * players, objects, event wrappers) is checked. Leaf values such as card faces and option lists are trusted: they
 * come from the generated bindings, which a Rust test keeps in step with the server.
 */
import type {
  AttackerView,
  Decision,
  ErrorCode,
  EventView,
  GameResult,
  ObjectView,
  PlayerView,
  ServerMessage,
  StateView,
  TableView,
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
  Decision: true,
  Events: true,
  Snapshot: true,
  Chat: true,
  Error: true,
  GameOver: true,
}

const ERROR_CODES: Record<ErrorCode, true> = {
  UnsupportedVersion: true,
  BadMessage: true,
  UnknownGame: true,
  StaleDecision: true,
  IllegalAnswer: true,
  Internal: true,
}

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
    (value.mana_cost === null || isRecord(value.mana_cost)) &&
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
      return {
        type,
        protocol_version: v.protocol_version,
        game: v.game,
        player: v.player,
        seats: v.seats,
        auto_pass: v.auto_pass,
      }
    case 'Decision':
      if (!isCount(v.id)) return '"id" must be a whole number'
      if (!isDecision(v.decision)) return '"decision" must name a player and exactly one decision kind'
      return { type, id: v.id, decision: v.decision }
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
    default: {
      // Compile-time exhaustiveness: a new `ServerMessage` variant stops building here until it is handled.
      const unhandled: never = type
      return `no reader for message type ${String(unhandled)}`
    }
  }
}
