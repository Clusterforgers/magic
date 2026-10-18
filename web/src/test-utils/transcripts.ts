import { isRecord } from '@/net/check'
import { isStateView, parseMessage } from '@/net/parse'
import fixtureJson from '@/protocol/fixtures/two-player-seat0.json'
import type { ServerMessage, StateView } from '@/protocol/generated'

/** One recorded history of a seat: what the server sent it, in order, and the view that is true at the end. */
export interface Transcript {
  name: string
  seat: number
  /** The messages as they were read at the boundary. */
  messages: ServerMessage[]
  /** The same messages as plain JSON, for comparisons with the wire. */
  raw: unknown[]
  expected: StateView
}

function read(name: string, seat: unknown, value: unknown): Transcript {
  if (!isRecord(value) || !Array.isArray(value.messages) || !isStateView(value.expected)) {
    throw new Error(`fixture ${name} is malformed`)
  }
  if (typeof seat !== 'number') throw new Error('fixture seat is malformed')
  const raw: unknown[] = value.messages
  const messages = raw.map((item, index) => {
    const parsed = parseMessage(item)
    if (!parsed.ok) throw new Error(`fixture ${name} message ${index}: ${parsed.error.detail}`)
    return parsed.message
  })
  return { name, seat, messages, raw, expected: value.expected }
}

const fixture: unknown = fixtureJson
if (!isRecord(fixture)) throw new Error('fixture is malformed')

/** Seat 0 joining a running game: Welcome, Snapshot, then the last batches and GameOver. */
export const lateJoin: Transcript = read('late join', fixture.seat, fixture)
/** Seat 0 present at the start: Welcome, then every batch from seq 0 with no Snapshot. */
export const lobbyStart: Transcript = read('lobby', fixture.seat, fixture.lobby)
/** Where the batch at seq 0 stands in `lobbyStart`: after the welcome and the rosters the lobby sent. */
export const lobbyBatchAt: number = lobbyStart.messages.findIndex((message) => message.type === 'Events')
/** Both transcripts, for `it.each`. */
export const transcripts: Transcript[] = [lateJoin, lobbyStart]
