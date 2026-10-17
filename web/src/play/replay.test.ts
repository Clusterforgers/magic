// Goal: both recorded transcripts, played through a real session on a fake socket, end in exactly the view the server says is true, with the right seat, log, phase and decisions.
import { describe, expect, it } from 'vitest'
import type { Decision, EventView, ServerMessage } from '@/protocol/generated'
import { startSession } from '@/test-utils/harness'
import { lateJoin, lobbyStart, transcripts } from '@/test-utils/transcripts'
import type { Transcript } from '@/test-utils/transcripts'

const decisions = (t: Transcript): { id: number; decision: Decision }[] =>
  t.messages.flatMap((m) => (m.type === 'Decision' ? [{ id: m.id, decision: m.decision }] : []))

const batches = (t: Transcript): Extract<ServerMessage, { type: 'Events' }>[] =>
  t.messages.flatMap((m) => (m.type === 'Events' ? [m] : []))

function play(t: Transcript) {
  const h = startSession()
  h.session.connect()
  h.open()
  h.deliver(...t.raw)
  return h
}

describe.each(transcripts)('the $name transcript', (t) => {
  it('ends in exactly the expected view, reachable by id', () => {
    const h = play(t)
    const state = h.state()
    expect(state.view).toEqual(t.expected)
    expect(Object.keys(state.objectsById)).toHaveLength(t.expected.objects.length)
    for (const object of state.view?.objects ?? []) expect(state.objectsById[object.id]).toBe(object)
  })

  it('knows its seat, is open, asked for nothing and has nothing to apologise for', () => {
    const h = play(t)
    const state = h.state()
    expect([state.seat, state.seatCount, state.status, state.attempt, state.notice]).toEqual([
      t.seat,
      2,
      'open',
      0,
      null,
    ])
    expect(h.sentOf('Resync')).toEqual([])
    expect(h.sentTypes()).toEqual(['Join'])
  })

  it('expects the batch after the last one', () => {
    const last = batches(t).at(-1)
    expect(play(t).state().seq).toBe(last && last.from_seq + last.events.length)
  })

  it('logs every event of every batch once, in order, with its position', () => {
    const state = play(t).state()
    const expected = batches(t).flatMap((b) =>
      b.events.map((event, i): [number, EventView] => [b.from_seq + i, event]),
    )
    expect(state.log.map((entry): [number, EventView] => [entry.seq, entry.event])).toEqual(expected)
    const seqs = state.log.map((entry) => entry.seq)
    expect(seqs).toEqual(seqs.map((_seq, i) => (seqs[0] ?? 0) + i))
  })

  it('answers every decision exactly once, in order, without disturbing the state', () => {
    const h = startSession()
    h.session.connect()
    h.open()
    for (const raw of t.raw) {
      h.deliver(raw)
      if (h.state().pending !== null) {
        expect(h.session.answer({ Priority: 'Pass' })).toBe(true)
        expect(h.session.answer({ Priority: 'Pass' })).toBe(false)
      }
    }
    expect(h.answeredIds()).toEqual(decisions(t).map((d) => d.id))
    expect(h.state().view).toEqual(t.expected)
  })

  it('reports the connection open only once the seat is known', () => {
    const h = startSession()
    const seen: [string, number | null][] = []
    h.session.store.subscribe((state) => seen.push([state.status, state.seat]))
    h.session.connect()
    h.open()
    h.deliver(...t.raw)
    const opened = seen.filter(([status]) => status === 'open')
    expect(opened.length).toBeGreaterThan(0)
    expect(opened.every(([, seat]) => seat === t.seat)).toBe(true)
  })
})

describe('the late-join transcript', () => {
  it('ends the game: phase over, the result set, no decision left to answer', () => {
    const h = play(lateJoin)
    const state = h.state()
    expect(state.phase).toBe('over')
    expect(state.result).toEqual(lateJoin.expected.result)
    expect(state.result).toEqual({ Winner: 0 })
    expect(state.pending).toBeNull()
    expect(h.session.answer({ Priority: 'Pass' })).toBe(false)
  })

  it('starts from the Snapshot: playing, with the snapshot position as the first expected seq', () => {
    const h = startSession()
    h.session.connect()
    h.open()
    const snapshot = lateJoin.messages.find((m) => m.type === 'Snapshot')
    const [welcome] = lateJoin.raw
    h.deliver(welcome)
    expect(h.state().phase).toBe('lobby')
    h.deliver(lateJoin.raw[1])
    expect(h.state().phase).toBe('playing')
    expect(h.state().seq).toBe(snapshot?.type === 'Snapshot' ? snapshot.seq : -1)
    expect(h.state().log).toEqual([])
  })
})

describe('the lobby transcript', () => {
  it('waits in the lobby until the batch at seq 0, then plays', () => {
    const h = startSession()
    h.session.connect()
    h.open()
    h.deliver(lobbyStart.raw[0])
    expect(h.state()).toMatchObject({ phase: 'lobby', view: null, seq: null, seat: 0 })
    h.deliver(lobbyStart.raw[1])
    const first = batches(lobbyStart)[0]
    expect(h.state()).toMatchObject({ phase: 'playing', seq: first?.events.length })
    expect(h.state().view).not.toBeNull()
  })

  it('is still playing at the end, with the last decision pending', () => {
    const state = play(lobbyStart).state()
    const last = decisions(lobbyStart).at(-1)
    expect(state.phase).toBe('playing')
    expect(state.result).toBeNull()
    expect(state.pending).toEqual(last)
  })

  it('has every object that left the seat view gone from the index too', () => {
    const state = play(lobbyStart).state()
    const everSeen = new Set(batches(lobbyStart).flatMap((b) => b.objects.map((o) => o.id)))
    const finalIds = new Set(lobbyStart.expected.objects.map((o) => o.id))
    const left = [...everSeen].filter((id) => !finalIds.has(id))
    expect(left.length).toBeGreaterThan(0)
    for (const id of left) expect(state.objectsById[id]).toBeUndefined()
  })
})
