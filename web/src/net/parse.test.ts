// Goal: every frame the server could send becomes a typed message or a typed FrameError; nothing malformed throws or slips through.
import { describe, expect, it } from 'vitest'
import { isRecord } from '@/net/check'
import { lateJoin, transcripts } from '@/test-utils/transcripts'
import { MAX_FRAME_CHARS, parseFrame, parseMessage } from './parse'

const wire = (value: unknown): string => JSON.stringify(value)

/** A copy of `node` with the value at `path` replaced; `undefined` removes the field on the wire. */
function put(node: unknown, path: readonly (string | number)[], value: unknown): unknown {
  const [head, ...rest] = path
  if (head === undefined) return value
  if (Array.isArray(node)) {
    const copy: unknown[] = [...node]
    copy[Number(head)] = put(node[Number(head)], rest, value)
    return copy
  }
  if (isRecord(node)) return { ...node, [head]: put(node[head], rest, value) }
  throw new Error(`no field ${String(head)} to replace`)
}

const first = (type: string): Record<string, unknown> => {
  const found = lateJoin.raw.find((item) => isRecord(item) && item.type === type)
  if (!isRecord(found)) throw new Error(`the fixture has no ${type}`)
  return found
}

const withObjects = lateJoin.raw.find(
  (item) =>
    isRecord(item) && item.type === 'Events' && Array.isArray(item.objects) && item.objects.length > 0,
)

/** One valid sample of every server message type. */
const samples: Record<string, Record<string, unknown>> = {
  Welcome: first('Welcome'),
  Snapshot: first('Snapshot'),
  Events: isRecord(withObjects) ? withObjects : first('Events'),
  Decision: first('Decision'),
  GameOver: first('GameOver'),
  Lobby: {
    type: 'Lobby',
    seats: [
      { seat: 0, name: 'Ana', connected: true, ready: false },
      { seat: 1, name: 'Bo', connected: false, ready: true },
    ],
    needed: 1,
  },
  Chat: { type: 'Chat', from: 1, text: 'good luck' },
  Error: { type: 'Error', code: 'StaleDecision', message: 'that decision is not pending for you' },
}

describe('parseFrame on real traffic', () => {
  it.each(transcripts)('$name: every recorded message is accepted and unchanged', ({ raw }) => {
    for (const item of raw) {
      const frame = parseFrame(wire(item))
      expect(frame.ok && frame.message).toEqual(item)
    }
  })

  it.each(Object.entries(samples))('%s: unknown extra fields are tolerated', (_type, sample) => {
    expect(parseFrame(wire({ ...sample, added_later: { anything: [1, 2] } })).ok).toBe(true)
  })
})

describe('parseFrame refuses what is not a message', () => {
  const unparsable: [string, unknown, string][] = [
    ['a binary frame', new ArrayBuffer(8), 'not_text'],
    ['a blob', new Blob(['{}']), 'not_text'],
    ['nothing', undefined, 'not_text'],
    ['null', null, 'not_text'],
    ['a number', 42, 'not_text'],
    ['an already decoded object', { type: 'Welcome' }, 'not_text'],
    ['an empty string', '', 'not_json'],
    ['a half object', '{"type":"Welcome"', 'not_json'],
    ['a trailing comma', '{"type":"Chat",}', 'not_json'],
    ['plain words', 'hello there', 'not_json'],
    ['undefined spelled out', 'undefined', 'not_json'],
    ['endless nesting', '['.repeat(50_000), 'not_json'],
    ['null as JSON', 'null', 'bad_shape'],
    ['a number as JSON', '42', 'bad_shape'],
    ['a string as JSON', '"Welcome"', 'bad_shape'],
    ['an array of messages', '[{"type":"Chat","from":0,"text":"x"}]', 'bad_shape'],
    ['an object without a type', '{}', 'bad_shape'],
    ['a numeric type', '{"type":7}', 'bad_shape'],
    ['a null type', '{"type":null}', 'bad_shape'],
    ['an unknown type', '{"type":"Hello"}', 'unknown_type'],
    ['a type in the wrong case', '{"type":"welcome"}', 'unknown_type'],
    ['a client message type', '{"type":"Join","protocol_version":1,"game":"g","name":"n"}', 'unknown_type'],
    ['an inherited property name', '{"type":"toString"}', 'unknown_type'],
    ['a prototype name', '{"type":"__proto__"}', 'unknown_type'],
  ]

  it.each(unparsable)('%s becomes a %s error, not an exception', (_name, data, kind) => {
    expect(parseFrame(data)).toEqual({ ok: false, error: { kind, detail: expect.any(String) } })
  })

  it('drops a frame over the size limit without reading it, and accepts one exactly at the limit', () => {
    const head = '{"type":"Chat","from":0,"text":"'
    const padded = (length: number): string => `${head}${'a'.repeat(length - head.length - 2)}"}`
    expect(parseFrame(padded(MAX_FRAME_CHARS)).ok).toBe(true)
    expect(parseFrame(padded(MAX_FRAME_CHARS + 1))).toMatchObject({ ok: false, error: { kind: 'too_large' } })
    expect(parseFrame('x'.repeat(MAX_FRAME_CHARS * 2))).toMatchObject({
      ok: false,
      error: { kind: 'too_large' },
    })
  })

  it('keeps an echoed unknown type short', () => {
    const frame = parseFrame(wire({ type: 'A'.repeat(5000) }))
    expect(!frame.ok && frame.error.detail.length).toBeLessThan(120)
  })
})

describe('parseFrame checks the fields of each message', () => {
  const missing = Object.entries(samples).flatMap(([type, sample]) =>
    Object.keys(sample)
      .filter((key) => key !== 'type')
      .map((key): [string, string] => [type, key]),
  )

  it.each(missing)('%s without "%s" is a bad_shape that names the field', (type, key) => {
    const frame = parseFrame(wire(put(samples[type], [key], undefined)))
    expect(frame).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
    expect(!frame.ok && frame.error.detail).toContain(`${type}: "${key}"`)
  })

  const wrongTypes: [string, string, string[], unknown][] = [
    ['Welcome', 'protocol_version as text', ['protocol_version'], '1'],
    ['Welcome', 'game as a number', ['game'], 7],
    ['Welcome', 'player as a fraction', ['player'], 0.5],
    ['Welcome', 'player as negative', ['player'], -1],
    ['Welcome', 'seats as null', ['seats'], null],
    ['Welcome', 'auto_pass as text', ['auto_pass'], 'on'],
    ['Welcome', 'auto_pass as a number', ['auto_pass'], 1],
    ['Welcome', 'auto_pass as null', ['auto_pass'], null],
    ['Lobby', 'seats as null', ['seats'], null],
    ['Lobby', 'seats as an object', ['seats'], {}],
    ['Lobby', 'a seat that is text', ['seats', '0'], 'Ana'],
    ['Lobby', 'a seat that is null', ['seats', '1'], null],
    ['Lobby', 'a seat number as text', ['seats', '0', 'seat'], '0'],
    ['Lobby', 'a seat number as a fraction', ['seats', '0', 'seat'], 0.5],
    ['Lobby', 'a seat number out of order', ['seats', '1', 'seat'], 5],
    ['Lobby', 'a seat number repeated', ['seats', '1', 'seat'], 0],
    ['Lobby', 'a name as a number', ['seats', '0', 'name'], 5],
    ['Lobby', 'a name as null', ['seats', '0', 'name'], null],
    ['Lobby', 'connected as text', ['seats', '0', 'connected'], 'yes'],
    ['Lobby', 'connected as a number', ['seats', '0', 'connected'], 1],
    ['Lobby', 'ready as null', ['seats', '0', 'ready'], null],
    ['Lobby', 'ready as text', ['seats', '1', 'ready'], 'true'],
    ['Lobby', 'needed as text', ['needed'], '1'],
    ['Lobby', 'needed as negative', ['needed'], -1],
    ['Lobby', 'needed as a fraction', ['needed'], 0.5],
    ['Lobby', 'needed as null', ['needed'], null],
    ['Decision', 'id as text', ['id'], '7'],
    ['Decision', 'id as negative', ['id'], -3],
    ['Decision', 'deadline_ms as text', ['deadline_ms'], '10000'],
    ['Decision', 'deadline_ms as a fraction', ['deadline_ms'], 99.5],
    ['Decision', 'deadline_ms as negative', ['deadline_ms'], -1],
    ['Decision', 'deadline_ms as null', ['deadline_ms'], null],
    ['Decision', 'decision as text', ['decision'], 'Priority'],
    ['Decision', 'player as text', ['decision', 'player'], 'me'],
    ['Decision', 'kind as text', ['decision', 'kind'], 'Priority'],
    ['Decision', 'kind with two variants', ['decision', 'kind'], { Priority: {}, Discard: {} }],
    ['Decision', 'kind with an empty body', ['decision', 'kind'], {}],
    ['Decision', 'kind with a non-object body', ['decision', 'kind'], { Priority: 3 }],
    ['Snapshot', 'seq as text', ['seq'], '3'],
    ['Snapshot', 'state as a list', ['state'], []],
    ['Snapshot', 'state without players', ['state', 'players'], undefined],
    ['Snapshot', 'state objects as an object', ['state', 'objects'], {}],
    ['Events', 'from_seq as a fraction', ['from_seq'], 1.5],
    ['Events', 'from_seq as negative', ['from_seq'], -1],
    ['Events', 'from_seq as text', ['from_seq'], '0'],
    ['Events', 'events as text', ['events'], 'none'],
    ['Events', 'an event that is a number', ['events', '0'], 5],
    ['Events', 'an event with no wrapper', ['events', '0'], {}],
    [
      'Events',
      'an event with both wrappers',
      ['events', '0'],
      { Public: 'MulliganPhaseBegan', Hidden: 'ObjectLeftGame' },
    ],
    ['Events', 'an event with an unusable body', ['events', '0'], { Public: 1 }],
    ['Events', 'table as null', ['table'], null],
    ['Events', 'table players as text', ['table', 'players'], 'x'],
    ['Events', 'a player seat as text', ['table', 'players', '0', 'seat'], 'a'],
    ['Events', 'a player life as a fraction', ['table', 'players', '0', 'life'], 1.5],
    ['Events', 'a hand without a count', ['table', 'players', '0', 'hand'], {}],
    ['Events', 'a hand with cards as text', ['table', 'players', '0', 'hand', 'cards'], 'abc'],
    ['Events', 'a mana pool of the wrong length', ['table', 'players', '0', 'mana', 'counts'], [1, 2]],
    ['Events', 'turn as null', ['table', 'turn'], null],
    ['Events', 'turn step as a number', ['table', 'turn', 'step'], 3],
    ['Events', 'priority as a number', ['table', 'priority'], 2],
    ['Events', 'battlefield as a list', ['table', 'battlefield'], []],
    ['Events', 'combat attackers as an object', ['table', 'combat', 'attackers'], {}],
    ['Events', 'result as an unknown word', ['table', 'result'], 'Won'],
    ['Events', 'result with a text winner', ['table', 'result'], { Winner: 'me' }],
    ['Events', 'viewer as negative', ['table', 'viewer'], -1],
    ['Events', 'objects as null', ['objects'], null],
    ['Events', 'an object id as text', ['objects', '0', 'id'], 'x'],
    ['Events', 'an object name as a number', ['objects', '0', 'name'], 5],
    ['Events', 'object types as text', ['objects', '0', 'types'], 'Creature'],
    ['Events', 'object power as text', ['objects', '0', 'power'], '2'],
    ['Events', 'an object cost as text', ['objects', '0', 'mana_cost'], '{1}{G}'],
    [
      'Events',
      'an object cost in the shape from before hybrid symbols',
      ['objects', '0', 'mana_cost'],
      { generic: 1, colorless: 0, colored: [0, 0, 0, 0, 1] },
    ],
    [
      'Events',
      'object permanent as junk',
      ['objects', '0', 'permanent'],
      { tapped: 'yes', damage: 0, summoning_sick: false },
    ],
    ['Events', 'gone with a fraction', ['gone'], [1.5]],
    ['Events', 'gone as a number', ['gone'], 3],
    ['Chat', 'text as a number', ['text'], 5],
    ['Chat', 'from as text', ['from'], 'me'],
    ['Error', 'an unknown code', ['code'], 'Nope'],
    ['Error', 'a numeric code', ['code'], 4],
    ['Error', 'a code from the prototype', ['code'], 'toString'],
    ['Error', 'message as null', ['message'], null],
    ['GameOver', 'an unknown word as result', ['result'], 'Win'],
    ['GameOver', 'a winner that is text', ['result'], { Winner: 'x' }],
    ['GameOver', 'a winner with company', ['result'], { Winner: 0, Draw: true }],
    ['GameOver', 'result as null', ['result'], null],
  ]

  it.each(wrongTypes)('%s with %s is a bad_shape error', (type, _what, path, value) => {
    const frame = parseFrame(wire(put(samples[type], path, value)))
    expect(frame).toMatchObject({
      ok: false,
      error: { kind: 'bad_shape', detail: expect.stringContaining(`${type}: `) },
    })
  })

  it('reads the roster exactly as the server writes it, and hands on every field unchanged', () => {
    // The text `crates/mtg-server/tests/ws_lobby.rs` asserts the server sends for a lobby of three seats.
    const text =
      '{"type":"Lobby","seats":[{"seat":0,"name":"Ada","connected":true,"ready":false},' +
      '{"seat":1,"name":"Bo","connected":false,"ready":false},' +
      '{"seat":2,"name":"Cy of the Many Long Names Indeed","connected":false,"ready":false}],"needed":3}'
    expect(parseFrame(text)).toEqual({
      ok: true,
      message: {
        type: 'Lobby',
        needed: 3,
        seats: [
          { seat: 0, name: 'Ada', connected: true, ready: false },
          { seat: 1, name: 'Bo', connected: false, ready: false },
          { seat: 2, name: 'Cy of the Many Long Names Indeed', connected: false, ready: false },
        ],
      },
    })
  })

  it('accepts a roster of any size the server can send, and names of any kind', () => {
    for (const count of [1, 2, 6, 12]) {
      const seats = Array.from({ length: count }, (_, seat) => ({
        seat,
        name: seat % 2 === 0 ? 'Ünïcode "quoted" \\ name' : '',
        connected: seat % 3 === 0,
        ready: seat % 2 === 0,
      }))
      expect(parseFrame(wire({ type: 'Lobby', seats, needed: count })).ok).toBe(true)
    }
  })

  it('keeps the deadline of a decision on a clock and leaves the field out of one without', () => {
    const clocked = { ...samples.Decision, deadline_ms: 10_000 }
    expect(parseFrame(wire(clocked))).toEqual({ ok: true, message: clocked })
    const plain = parseFrame(wire(samples.Decision))
    expect(plain.ok && 'deadline_ms' in plain.message).toBe(false)
    expect(parseFrame(wire({ ...samples.Decision, deadline_ms: 0 })).ok).toBe(true)
  })

  it('accepts a draw and a winner as results', () => {
    for (const result of ['Draw', { Winner: 3 }]) {
      expect(parseFrame(wire({ type: 'GameOver', result })).ok).toBe(true)
    }
  })

  it('reads decoded JSON the same way as a raw frame', () => {
    expect(parseMessage(samples.Chat)).toEqual({ ok: true, message: samples.Chat })
    expect(parseMessage('Chat')).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
  })
})
