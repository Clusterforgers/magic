// Goal: the transcript fixture parses as server messages of the documented shape, and the real session turns it into the view the server says is true.
import { describe, expect, it } from 'vitest'
import { startSession } from '@/test-utils/harness'
import fixtureJson from './fixtures/two-player-seat0.json'
import { PROTOCOL_VERSION } from './generated'
import type { ServerMessage, StateView } from './generated'

interface Transcript {
  seed: number
  messages: ServerMessage[]
  expected: StateView
}

interface Fixture extends Transcript {
  description: string
  seat: number
  lobby: Transcript
}

const fixture = fixtureJson as unknown as Fixture
const transcripts: [string, Transcript][] = [
  ['late join', fixture],
  ['lobby', fixture.lobby],
]

const TABLE_KEYS = [
  'battlefield',
  'combat',
  'combat_forecast',
  'exile',
  'first_player',
  'format',
  'players',
  'priority',
  'result',
  'stack',
  'stage',
  'turn',
  'viewer',
]

const keysOf = (value: object): string[] => Object.keys(value).sort()

/** The client rule, played by the real session on a fake socket: the view it ends with and how often it asked for a resync. */
const replay = (messages: ServerMessage[]) => {
  const h = startSession()
  h.session.connect()
  h.open()
  h.deliver(...messages)
  const { view, seq } = h.state()
  return { state: view, nextSeq: seq, resyncs: h.sentOf('Resync').length }
}

describe('transcript fixture', () => {
  it('is two transcripts of seat 0, each with 40 batches', () => {
    expect(fixture.seat).toBe(0)
    for (const [name, transcript] of transcripts) {
      const batches = transcript.messages.filter((m) => m.type === 'Events')
      expect(batches.length, name).toBe(40)
    }
  })

  it('opens a late join with Welcome and Snapshot, and ends it with GameOver', () => {
    const [welcome, snapshot, ...rest] = fixture.messages
    expect(welcome).toEqual({
      type: 'Welcome',
      protocol_version: PROTOCOL_VERSION,
      game: expect.any(String),
      player: 0,
      seats: 2,
      auto_pass: false,
      yielding: null,
      stops: [],
      full_control: 'Off',
    })
    expect(snapshot?.type).toBe('Snapshot')
    expect(rest.at(-1)?.type).toBe('GameOver')
    expect(fixture.expected.result).not.toBeNull()
  })

  it('opens a lobby transcript with the welcome, the lobby rosters and the batch at seq 0, and no Snapshot', () => {
    const [welcome, ...rest] = fixture.lobby.messages
    expect(welcome?.type).toBe('Welcome')
    const at = rest.findIndex((m) => m.type === 'Events')
    expect(at).toBeGreaterThan(0)
    expect(rest.slice(0, at).every((m) => m.type === 'Lobby')).toBe(true)
    const first = rest[at]
    expect(first?.type === 'Events' && first.from_seq).toBe(0)
    expect(rest.slice(at).some((m) => m.type === 'Lobby')).toBe(false)
    expect(fixture.lobby.messages.some((m) => m.type === 'Snapshot')).toBe(false)
  })

  it('has rosters that read as one table filling up: both seats named, needed following the ready seats', () => {
    const rosters = fixture.lobby.messages.flatMap((m) => (m.type === 'Lobby' ? [m] : []))
    expect(rosters.length).toBeGreaterThanOrEqual(3)
    for (const roster of rosters) {
      expect(roster.seats.map((seat) => seat.seat)).toEqual([0, 1])
      expect(roster.needed).toBe(roster.seats.filter((seat) => !seat.ready).length)
    }
    // Seat 1 joins, is ready, takes it back and is ready again: the flags move one at a time.
    expect(rosters.map((roster) => roster.seats.map((seat) => [seat.connected, seat.ready]))).toEqual([
      [
        [true, false],
        [false, false],
      ],
      [
        [true, false],
        [true, false],
      ],
      [
        [true, false],
        [true, true],
      ],
      [
        [true, false],
        [true, false],
      ],
      [
        [true, false],
        [true, true],
      ],
    ])
  })

  it.each(transcripts)('%s: every message has the shape the bindings describe', (_name, transcript) => {
    const seen = new Set<string>()
    for (const message of transcript.messages) {
      seen.add(message.type)
      switch (message.type) {
        case 'Welcome':
          expect(keysOf(message)).toEqual([
            'auto_pass',
            'full_control',
            'game',
            'player',
            'protocol_version',
            'seats',
            'stops',
            'type',
            'yielding',
          ])
          break
        case 'Snapshot':
          expect(keysOf(message)).toEqual(['seq', 'state', 'type'])
          expect(keysOf(message.state)).toEqual([...TABLE_KEYS, 'objects'].sort())
          break
        case 'Events':
          expect(keysOf(message)).toEqual(['events', 'from_seq', 'gone', 'objects', 'table', 'type'])
          expect(keysOf(message.table)).toEqual(TABLE_KEYS)
          for (const event of message.events) {
            expect(Object.keys(event)).toHaveLength(1)
            expect(['Public', 'Hidden']).toContain(Object.keys(event)[0])
          }
          break
        case 'Decision':
          expect(keysOf(message)).toEqual(['decision', 'id', 'type'])
          expect(message.decision.player).toBe(0)
          expect(Object.keys(message.decision.kind)).toHaveLength(1)
          break
        case 'GameOver':
          expect(keysOf(message)).toEqual(['result', 'type'])
          break
        case 'Lobby':
          // Only a seat that was there before the start is sent rosters.
          expect(keysOf(message)).toEqual(['needed', 'seats', 'type'])
          for (const seat of message.seats)
            expect(keysOf(seat)).toEqual(['connected', 'name', 'ready', 'seat'])
          break
        default:
          expect.unreachable(`a transcript never carries a ${message.type} message`)
      }
    }
    expect(seen).toContain('Decision')
    expect(seen).toContain('Events')
  })

  it.each(transcripts)('%s: decision ids rise and batches follow one another', (_name, transcript) => {
    const ids = transcript.messages.flatMap((m) => (m.type === 'Decision' ? [m.id] : []))
    expect(ids.length).toBeGreaterThan(5)
    expect(ids).toEqual([...ids].sort((a, b) => a - b))
    expect(new Set(ids).size).toBe(ids.length)
    expect(replay(transcript.messages).resyncs).toBe(0)
  })

  it.each(transcripts)(
    '%s: folding the messages gives the view the server says is true',
    (_name, transcript) => {
      const { state, nextSeq } = replay(transcript.messages)
      expect(state).toEqual(transcript.expected)
      const last = transcript.messages.findLast((m) => m.type === 'Events')
      expect(last?.type === 'Events' && last.from_seq + last.events.length).toBe(nextSeq)
      const ids = transcript.expected.objects.map((o) => o.id)
      expect(ids).toEqual([...ids].sort((a, b) => a - b))
    },
  )

  it('shows objects leaving the seat view in the lobby transcript', () => {
    const gone = fixture.lobby.messages.flatMap((m) => (m.type === 'Events' ? m.gone : []))
    expect(gone.length).toBeGreaterThan(0)
    const finalIds = new Set(fixture.lobby.expected.objects.map((o) => o.id))
    expect(gone.some((id) => !finalIds.has(id))).toBe(true)
  })

  it('notices a dropped message as a gap', () => {
    const second = fixture.messages.findIndex((m, i) => i > 1 && m.type === 'Events')
    const dropped = fixture.messages.filter((_m, i) => i !== second)
    expect(dropped).toHaveLength(fixture.messages.length - 1)
    expect(replay(dropped).resyncs).toBe(1)
  })
})
