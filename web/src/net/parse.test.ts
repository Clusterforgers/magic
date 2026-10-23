// Goal: every frame the server could send becomes a typed message or a typed FrameError; nothing malformed throws or slips through.
import { describe, expect, it } from 'vitest'
import { isRecord } from '@/net/check'
import { KINDS } from '@/test-utils/decisions'
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

/** Every step of a turn, as the server names them. */
const STEPS = [
  'Untap',
  'Upkeep',
  'Draw',
  'Main1',
  'BeginCombat',
  'DeclareAttackers',
  'DeclareBlockers',
  'FirstStrikeDamage',
  'CombatDamage',
  'EndCombat',
  'Main2',
  'End',
  'Cleanup',
]
const SIDES = ['Own', 'Opponent']
const MODES = ['Off', 'NextWindow', 'Held']
// Lasting and one-turn stops alternate, so both kinds are on every side.
const everyStop = SIDES.flatMap((side) => STEPS.map((step, n) => ({ side, step, keep: n % 2 === 0 })))

/** One valid sample of every server message type. */
const samples: Record<string, Record<string, unknown>> = {
  // The recorded welcome holds no stops; this one holds two, so the checks reach into the list.
  Welcome: {
    ...first('Welcome'),
    stops: [
      { side: 'Own', step: 'Main1', keep: true },
      { side: 'Opponent', step: 'End', keep: false },
    ],
    full_control: 'Held',
  },
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
  Yielding: { type: 'Yielding', until: 'EndOfTurn' },
  Stops: {
    type: 'Stops',
    stops: [
      { side: 'Own', step: 'Upkeep', keep: false },
      { side: 'Opponent', step: 'DeclareBlockers', keep: true },
    ],
  },
  FullControl: { type: 'FullControl', mode: 'NextWindow' },
  Bank: { type: 'Bank', seat: 1, bank: 2 },
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
    ['Welcome', 'yielding as an unknown word', ['yielding'], 'Soon'],
    ['Welcome', 'yielding in lower case', ['yielding'], 'endofturn'],
    ['Welcome', 'yielding as a number', ['yielding'], 1],
    ['Welcome', 'yielding as a flag', ['yielding'], true],
    ['Welcome', 'yielding as a prototype name', ['yielding'], 'toString'],
    ['Yielding', 'until as an unknown word', ['until'], 'Soon'],
    ['Yielding', 'until in lower case', ['until'], 'mine'],
    ['Yielding', 'until as a number', ['until'], 0],
    ['Yielding', 'until as an object', ['until'], { EndOfTurn: true }],
    ['Yielding', 'until as a prototype name', ['until'], '__proto__'],
    ['Welcome', 'stops as null', ['stops'], null],
    ['Welcome', 'stops as one stop', ['stops'], { side: 'Own', step: 'Main1', keep: true }],
    ['Welcome', 'stops as text', ['stops'], 'Main1'],
    ['Welcome', 'a stop that is text', ['stops', '0'], 'Main1'],
    ['Welcome', 'a stop that is null', ['stops', '1'], null],
    ['Welcome', 'a stop without a side', ['stops', '0', 'side'], undefined],
    ['Welcome', 'a stop without a step', ['stops', '0', 'step'], undefined],
    ['Welcome', 'a side that is unknown', ['stops', '0', 'side'], 'Both'],
    ['Welcome', 'a side in lower case', ['stops', '0', 'side'], 'own'],
    ['Welcome', 'a side as a number', ['stops', '0', 'side'], 0],
    ['Welcome', 'a side from the prototype', ['stops', '0', 'side'], 'toString'],
    ['Welcome', 'a step that is unknown', ['stops', '0', 'step'], 'Combat'],
    ['Welcome', 'a step in lower case', ['stops', '0', 'step'], 'main1'],
    ['Welcome', 'a step with a space', ['stops', '0', 'step'], 'Main 1'],
    ['Welcome', 'a step as a number', ['stops', '0', 'step'], 3],
    ['Welcome', 'a step from the prototype', ['stops', '0', 'step'], '__proto__'],
    ['Welcome', 'a stop without keep', ['stops', '0', 'keep'], undefined],
    ['Welcome', 'keep as text', ['stops', '0', 'keep'], 'yes'],
    ['Welcome', 'keep as a number', ['stops', '1', 'keep'], 1],
    ['Welcome', 'keep as null', ['stops', '1', 'keep'], null],
    ['Welcome', 'a stop listed twice', ['stops', '1'], { side: 'Own', step: 'Main1', keep: true }],
    [
      'Welcome',
      'a stop listed twice with another keep',
      ['stops', '1'],
      { side: 'Own', step: 'Main1', keep: false },
    ],
    ['Welcome', 'full_control as an unknown word', ['full_control'], 'Always'],
    ['Welcome', 'full_control in lower case', ['full_control'], 'held'],
    ['Welcome', 'full_control as a flag', ['full_control'], true],
    ['Welcome', 'full_control as null', ['full_control'], null],
    ['Welcome', 'full_control as a prototype name', ['full_control'], 'constructor'],
    ['Stops', 'stops as null', ['stops'], null],
    ['Stops', 'stops as one stop', ['stops'], { side: 'Own', step: 'Main1', keep: true }],
    ['Stops', 'a stop that is text', ['stops', '0'], 'Upkeep'],
    ['Stops', 'a side that is unknown', ['stops', '0', 'side'], 'Both'],
    ['Stops', 'a step that is unknown', ['stops', '1', 'step'], 'Combat'],
    ['Stops', 'a step in lower case', ['stops', '1', 'step'], 'declareblockers'],
    ['Stops', 'a stop without a step', ['stops', '1', 'step'], undefined],
    ['Stops', 'a stop without keep', ['stops', '1', 'keep'], undefined],
    ['Stops', 'keep as text', ['stops', '0', 'keep'], 'false'],
    ['Stops', 'keep as a number', ['stops', '1', 'keep'], 0],
    ['Stops', 'keep as null', ['stops', '0', 'keep'], null],
    ['Stops', 'keep as an object', ['stops', '0', 'keep'], { value: true }],
    ['Stops', 'a stop listed twice', ['stops', '1'], { side: 'Own', step: 'Upkeep', keep: false }],
    [
      'Stops',
      'a stop listed twice with another keep',
      ['stops', '1'],
      { side: 'Own', step: 'Upkeep', keep: true },
    ],
    ['FullControl', 'mode as an unknown word', ['mode'], 'Always'],
    ['FullControl', 'mode in lower case', ['mode'], 'nextwindow'],
    ['FullControl', 'mode as a flag', ['mode'], false],
    ['FullControl', 'mode as null', ['mode'], null],
    ['FullControl', 'mode as a number', ['mode'], 1],
    ['FullControl', 'mode as an object', ['mode'], { Held: true }],
    ['FullControl', 'mode as a prototype name', ['mode'], 'hasOwnProperty'],
    ['Decision', 'id as text', ['id'], '7'],
    ['Decision', 'id as negative', ['id'], -3],
    ['Decision', 'deadline_ms as text', ['deadline_ms'], '10000'],
    ['Decision', 'deadline_ms as a fraction', ['deadline_ms'], 99.5],
    ['Decision', 'deadline_ms as negative', ['deadline_ms'], -1],
    ['Decision', 'deadline_ms as null', ['deadline_ms'], null],
    ['Decision', 'show_after_ms as text', ['show_after_ms'], '3000'],
    ['Decision', 'show_after_ms as a fraction', ['show_after_ms'], 0.5],
    ['Decision', 'show_after_ms as negative', ['show_after_ms'], -1],
    ['Decision', 'show_after_ms as null', ['show_after_ms'], null],
    ['Bank', 'seat as text', ['seat'], '1'],
    ['Bank', 'seat as negative', ['seat'], -1],
    ['Bank', 'bank as text', ['bank'], '2'],
    ['Bank', 'bank as a fraction', ['bank'], 1.5],
    ['Bank', 'bank as null', ['bank'], null],
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
    ['Events', 'the turn without an active player as text', ['table', 'turn', 'active_left'], 'yes'],
    ['Events', 'the turn without an active player as a number', ['table', 'turn', 'active_left'], 1],
    ['Events', 'the turn without an active player as null', ['table', 'turn', 'active_left'], null],
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

  it('keeps the delay before the countdown shows and leaves the field out when there is none', () => {
    const delayed = { ...samples.Decision, deadline_ms: 10_000, show_after_ms: 3_000 }
    expect(parseFrame(wire(delayed))).toEqual({ ok: true, message: delayed })
    const plain = parseFrame(wire({ ...samples.Decision, deadline_ms: 10_000 }))
    expect(plain.ok && 'show_after_ms' in plain.message).toBe(false)
  })

  it('accepts a draw and a winner as results', () => {
    for (const result of ['Draw', { Winner: 3 }]) {
      expect(parseFrame(wire({ type: 'GameOver', result })).ok).toBe(true)
    }
  })

  it.each([null, 'Resolve', 'EndOfTurn', 'MyTurn'])(
    'accepts %s as the yield of a Welcome and of a Yielding, and hands it on unchanged',
    (yielding) => {
      const welcome = { ...samples.Welcome, yielding }
      expect(parseFrame(wire(welcome))).toEqual({ ok: true, message: welcome })
      const notice = { type: 'Yielding', until: yielding }
      expect(parseFrame(wire(notice))).toEqual({ ok: true, message: notice })
    },
  )

  it('does not take a missing yield for none: the server always says which', () => {
    const { yielding: _yielding, ...without } = samples.Welcome ?? {}
    expect(parseFrame(wire(without))).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
    expect(parseFrame(wire({ type: 'Yielding' }))).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
  })

  it('names the yield field in the reason', () => {
    const frame = parseFrame(wire({ type: 'Yielding', until: 'Soon' }))
    expect(!frame.ok && frame.error.detail).toMatch(
      /Yielding: "until" must be Resolve, EndOfTurn, MyTurn or null/,
    )
  })

  it('accepts every side and step as a stop, on its own and in a list, and hands it on unchanged', () => {
    // The server never lists Untap or Cleanup: whether a step can hold a stop is its rule, so the page only
    // checks that a step is one.
    for (const stops of [[], [everyStop[0]], everyStop]) {
      const welcome = { ...samples.Welcome, stops }
      expect(parseFrame(wire(welcome))).toEqual({ ok: true, message: welcome })
      const notice = { type: 'Stops', stops }
      expect(parseFrame(wire(notice))).toEqual({ ok: true, message: notice })
    }
    for (const stop of everyStop) {
      expect(parseFrame(wire({ type: 'Stops', stops: [stop] })).ok, JSON.stringify(stop)).toBe(true)
    }
  })

  it.each(MODES)('accepts %s as the Full Control mode of a Welcome and of a FullControl', (mode) => {
    const welcome = { ...samples.Welcome, full_control: mode }
    expect(parseFrame(wire(welcome))).toEqual({ ok: true, message: welcome })
    const notice = { type: 'FullControl', mode }
    expect(parseFrame(wire(notice))).toEqual({ ok: true, message: notice })
  })

  it('takes the same step on the two sides for two stops, and a side and step twice for a mistake', () => {
    const both = [
      { side: 'Own', step: 'Main1', keep: true },
      { side: 'Opponent', step: 'Main1', keep: true },
    ]
    expect(parseFrame(wire({ type: 'Stops', stops: both })).ok).toBe(true)
    expect(parseFrame(wire({ type: 'Stops', stops: [...both, both[0]] })).ok).toBe(false)
    // Whatever keep the repeat has: a seat holds one stop at a side and step.
    const other = { side: 'Own', step: 'Main1', keep: false }
    expect(parseFrame(wire({ type: 'Stops', stops: [...both, other] })).ok).toBe(false)
  })

  it('hands on the keep of every stop as the server said it, lasting or not', () => {
    const lasting = { side: 'Own', step: 'Main1', keep: true }
    const oneTurn = { side: 'Opponent', step: 'End', keep: false }
    for (const stops of [[lasting], [oneTurn], [lasting, oneTurn], [oneTurn, lasting]]) {
      const notice = { type: 'Stops', stops }
      expect(parseFrame(wire(notice))).toEqual({ ok: true, message: notice })
    }
  })

  it('does not take a stop without a keep for a one-turn stop: the server always says which', () => {
    const { keep: _keep, ...bare } = { side: 'Own', step: 'Main1', keep: true }
    for (const bad of [
      { type: 'Stops', stops: [bare] },
      { ...samples.Welcome, stops: [bare] },
    ]) {
      expect(parseFrame(wire(bad))).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
    }
  })

  it('does not take a missing list for none or a missing mode for Off: the server always says which', () => {
    const { stops: _stops, ...noStops } = samples.Welcome ?? {}
    const { full_control: _mode, ...noMode } = samples.Welcome ?? {}
    for (const bad of [noStops, noMode, { type: 'Stops' }, { type: 'FullControl' }]) {
      expect(parseFrame(wire(bad))).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
    }
  })

  it('names the stops and the mode in the reason', () => {
    const stops = parseFrame(wire({ type: 'Stops', stops: [{ side: 'Own', step: 'Combat', keep: true }] }))
    expect(!stops.ok && stops.error.detail).toMatch(
      /Stops: "stops" must be a list of stops \(side, step, keep\), each side and step once/,
    )
    const mode = parseFrame(wire({ type: 'FullControl', mode: 'Always' }))
    expect(!mode.ok && mode.error.detail).toMatch(/FullControl: "mode" must be Off, NextWindow or Held/)
    const welcome = parseFrame(wire({ ...samples.Welcome, full_control: 'Always' }))
    expect(!welcome.ok && welcome.error.detail).toMatch(
      /Welcome: "full_control" must be Off, NextWindow or Held/,
    )
  })

  it('reads decoded JSON the same way as a raw frame', () => {
    expect(parseMessage(samples.Chat)).toEqual({ ok: true, message: samples.Chat })
    expect(parseMessage('Chat')).toMatchObject({ ok: false, error: { kind: 'bad_shape' } })
  })
})

describe('parseFrame on what spells and abilities add to the wire', () => {
  const snapshot = {
    id: 5,
    counter: 0,
    controller: 0,
    owner: 0,
    zone: 'Battlefield',
    characteristics: { types: 4, colors: 0, power: 1, toughness: 1, abilities: [] },
    counters: [],
    attachments: [],
  }
  const events = [
    { Public: { SpellCast: { player: 0, object: 4, choices: { targets: [[{ Player: 1 }]] } } } },
    {
      Public: {
        AbilityPutOnStack: {
          object: 90,
          source: { id: 5, counter: 0 },
          snapshot,
          ability: 0,
          controller: 1,
          choices: { targets: [[{ Object: { id: 3, counter: 1 } }]], x: 2 },
        },
      },
    },
    { Public: { AbilityActivated: { player: 1, source: { id: 5, counter: 0 }, ability: 0 } } },
    { Public: { ResolutionBegan: { object: 90 } } },
    { Public: { ChoiceMade: { player: 1, choice: { Cards: [3, 4] } } } },
    {
      Public: {
        StepDone: { position: [1, 0, 2], bound: [['found', [{ Player: 1 }]]], snapshots: [snapshot] },
      },
    },
    { Public: { ResolutionEnded: { object: 90, outcome: 'Fizzled' } } },
    { Public: { ZoneChanged: { object: 3, from: 'Battlefield', to: { Graveyard: 0 }, cause: 'Destroyed' } } },
    { Public: { LifeGained: { player: 0, amount: 3 } } },
    { Public: { LifeLost: { player: 1, amount: 2 } } },
    { Hidden: { LibraryLooked: { player: 1, count: 2 } } },
    { Hidden: { Scried: { player: 1, top: 1, bottom: 1 } } },
    { Hidden: { ZoneChanged: { from: { Library: 1 }, to: { Hand: 1 }, cause: 'Moved' } } },
    { Hidden: { ChoiceMade: { player: 1 } } },
    { Hidden: { StepDone: { position: [2] } } },
  ]
  const ability = {
    id: 90,
    owner: 1,
    controller: 1,
    zone: 'Stack',
    name: 'Pinger',
    mana_cost: null,
    supertypes: [],
    types: [],
    subtypes: [],
    colors: [],
    power: null,
    toughness: null,
    abilities: [],
    permanent: null,
    kind: 'Ability',
    source: 5,
    targets: [[{ Player: 0 }]],
  }

  it('accepts a decision of every kind, the effects’ ones included, and hands it on unchanged', () => {
    for (const [name, kind] of Object.entries(KINDS)) {
      const message = { type: 'Decision', id: 3, decision: { player: 0, kind } }
      expect(parseFrame(wire(message)), name).toEqual({ ok: true, message })
    }
  })

  it('accepts the events of spells and abilities, public and hidden, in a batch', () => {
    const batch = put(samples.Events, ['events'], events)
    expect(parseFrame(wire(batch))).toEqual({ ok: true, message: batch })
  })

  it('accepts an ability on the stack among the objects of a batch, with its source and targets', () => {
    const batch = put(samples.Events, ['objects'], [ability])
    expect(parseFrame(wire(batch))).toEqual({ ok: true, message: batch })
  })

  it('accepts a turn without an active player and one with, as the server writes them', () => {
    for (const left of [true, undefined]) {
      const batch = put(samples.Events, ['table', 'turn', 'active_left'], left)
      expect(parseFrame(wire(batch))).toMatchObject({ ok: true })
    }
  })

  it('accepts a card without the fields an ability adds, as the server writes it', () => {
    const card = { ...ability, kind: undefined, source: undefined, targets: undefined }
    const batch = put(samples.Events, ['objects'], [card])
    expect(parseFrame(wire(batch))).toMatchObject({ ok: true })
  })

  it('accepts the names of an object’s keywords, and an object without the field, as the server writes it', () => {
    const flyer = {
      ...ability,
      kind: undefined,
      source: undefined,
      targets: undefined,
      keywords: ['Flying', 'First strike'],
    }
    const batch = put(samples.Events, ['objects'], [flyer])
    expect(parseFrame(wire(batch))).toMatchObject({ ok: true })
    const none = { ...flyer, keywords: undefined }
    expect(parseFrame(wire(put(samples.Events, ['objects'], [none])))).toMatchObject({ ok: true })
  })

  it.each([
    ['a kind that is none of the three', { kind: 'Spell' }],
    ['a kind that is not text', { kind: 2 }],
    ['a source that is not an id', { source: -1 }],
    ['a source that is not a number', { source: 'Pinger' }],
    ['targets that are not a list of lists', { targets: [{ Player: 0 }] }],
    ['targets that are not a list', { targets: 'none' }],
    ['keywords that are text', { keywords: 'Flying' }],
    ['keywords that are not all text', { keywords: ['Flying', 2] }],
    ['keywords that are null', { keywords: null }],
  ])('refuses an object with %s', (_what, change) => {
    const batch = put(samples.Events, ['objects'], [{ ...ability, ...change }])
    expect(parseFrame(wire(batch))).toMatchObject({
      ok: false,
      error: { kind: 'bad_shape', detail: expect.stringContaining('objects') },
    })
  })
})
