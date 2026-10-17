// Goal: a batch whose from_seq is not the expected one sends exactly one Resync and applies nothing until the next Snapshot, which recovers the session.
import { describe, expect, it } from 'vitest'
import type { StateView } from '@/protocol/generated'
import { asRecord, must } from '@/test-utils/batches'
import { reference } from '@/test-utils/foldChecks'
import { startSession } from '@/test-utils/harness'
import { lateJoin, lobbyStart } from '@/test-utils/transcripts'
import type { Transcript } from '@/test-utils/transcripts'

/** The true view and log position after the first `count` messages of a transcript, by the plain reference fold. */
function truth(t: Transcript, count: number): { state: StateView; seq: number } {
  let state: StateView | null = null
  let seq = 0
  for (const message of t.messages.slice(0, count)) {
    if (message.type === 'Snapshot') {
      state = message.state
      seq = message.seq
    } else if (message.type === 'Events') {
      state = reference(state, message)
      seq = message.from_seq + message.events.length
    }
  }
  return { state: must(state, 'state'), seq }
}

// Messages 0 to 4 of the late-join transcript are Welcome, Snapshot, two batches and a Decision; message 5 is the next batch.
const APPLIED = 5
const next = asRecord(lateJoin.raw[APPLIED], 'the next batch')

/** The next real batch with its `from_seq` replaced. */
const withSeq = (fromSeq: number): unknown => ({ ...next, from_seq: fromSeq })

function atGap() {
  const h = startSession()
  h.session.connect()
  h.open()
  h.deliver(...lateJoin.raw.slice(0, APPLIED))
  const before = h.state()
  const expected = must(before.seq, 'seq')
  return { h, before, expected }
}

describe('a batch that does not follow the last one', () => {
  const offsets: [string, (expected: number) => number][] = [
    ['one ahead', (expected) => expected + 1],
    ['far ahead', (expected) => expected + 1000],
    ['two behind, inside an earlier batch', (expected) => expected - 2],
    ['a repeat of the batch before the last', (expected) => expected - 3],
    ['a repeat of the last batch', (expected) => expected - 1],
    ['position zero although there is a view', () => 0],
    ['the largest safe position', () => Number.MAX_SAFE_INTEGER],
  ]

  it('starts from the Snapshot and two applied batches', () => {
    const { before } = atGap()
    expect(before.phase).toBe('playing')
    expect(before.pending?.id).toBe(390)
    expect(before.seq).toBe(995)
  })

  it.each(offsets)('%s: one Resync, nothing applied', (_name, fromSeq) => {
    const { h, before, expected } = atGap()
    h.deliver(withSeq(fromSeq(expected)))
    expect(h.sentOf('Resync')).toEqual([{ type: 'Resync' }])
    const after = h.state()
    expect(after.view).toBe(before.view)
    expect(after.objectsById).toBe(before.objectsById)
    expect(after.log).toBe(before.log)
    expect(after.seq).toBe(expected)
    expect(after.notice).toBeNull()
  })

  it.each(offsets)(
    '%s: later batches, right or wrong, send no second Resync and apply nothing',
    (_name, fromSeq) => {
      const { h, before, expected } = atGap()
      h.deliver(withSeq(fromSeq(expected)), withSeq(expected + 7), withSeq(0))
      // Even the batch that would have been next in order waits for the Snapshot.
      h.deliver(next, ...lateJoin.raw.slice(APPLIED + 1, APPLIED + 4))
      expect(h.sentOf('Resync')).toHaveLength(1)
      expect(h.state().view).toBe(before.view)
      expect(h.state().seq).toBe(expected)
    },
  )

  it('ignores a Decision while it waits, because the server repeats the pending one behind the Snapshot', () => {
    const { h, before, expected } = atGap()
    h.deliver(withSeq(expected + 1))
    h.deliver({ type: 'Decision', id: 391, decision: before.pending?.decision })
    expect(h.state().pending?.id).toBe(390)
  })

  it('still takes chat and the end of the game while it waits', () => {
    const { h, expected } = atGap()
    h.deliver(
      withSeq(expected + 1),
      { type: 'Chat', from: 1, text: 'hello' },
      { type: 'GameOver', result: 'Draw' },
    )
    expect(h.state().chat).toEqual([{ from: 1, text: 'hello' }])
    expect(h.state()).toMatchObject({ result: 'Draw', phase: 'over', pending: null })
    expect(h.sentOf('Resync')).toHaveLength(1)
  })

  it('recovers when the Snapshot arrives: new view, new position, the repeated Decision, then batches in order', () => {
    const { h, before, expected } = atGap()
    h.deliver(withSeq(expected + 1))
    // The server's answer to the Resync: the snapshot after the batch that was missed, then the decision again.
    const recovered = truth(lateJoin, APPLIED + 1)
    h.deliver({ type: 'Snapshot', seq: recovered.seq, state: recovered.state })
    expect(h.state()).toMatchObject({ seq: recovered.seq, pending: null, phase: 'playing' })
    expect(h.state().view).toEqual(recovered.state)
    h.deliver({ type: 'Decision', id: 390, decision: before.pending?.decision })
    expect(h.state().pending?.id).toBe(390)
    h.deliver(...lateJoin.raw.slice(APPLIED + 1))
    expect(h.state().view).toEqual(lateJoin.expected)
    expect(h.state().phase).toBe('over')
    expect(h.sentOf('Resync')).toHaveLength(1)
  })

  it('can fall behind again after recovering, and asks again', () => {
    const { h, expected } = atGap()
    h.deliver(withSeq(expected + 1))
    const recovered = truth(lateJoin, APPLIED + 1)
    h.deliver({ type: 'Snapshot', seq: recovered.seq, state: recovered.state })
    h.deliver(withSeq(recovered.seq + 50))
    expect(h.sentOf('Resync')).toHaveLength(2)
    expect(h.state().seq).toBe(recovered.seq)
  })

  it('keeps the unchanged parts of the view when the Snapshot repeats what it already had', () => {
    const { h, before, expected } = atGap()
    h.deliver(withSeq(expected + 1))
    const sameState = truth(lateJoin, APPLIED)
    h.deliver({ type: 'Snapshot', seq: sameState.seq, state: sameState.state })
    expect(h.state().view).toBe(before.view)
    expect(h.state().objectsById).toBe(before.objectsById)
  })
})

describe('a batch with no view yet', () => {
  const lobbyStartOnly = (): ReturnType<typeof startSession> => {
    const h = startSession()
    h.session.connect()
    h.open()
    h.deliver(lobbyStart.raw[0])
    return h
  }

  it.each([6, 1, 100])(
    'at position %i, instead of the batch at 0, asks once and applies nothing',
    (fromSeq) => {
      const h = lobbyStartOnly()
      const batch = asRecord(lobbyStart.raw[3], 'the second batch')
      h.deliver({ ...batch, from_seq: fromSeq }, batch, lobbyStart.raw[1])
      expect(h.sentOf('Resync')).toHaveLength(1)
      expect(h.state()).toMatchObject({ view: null, seq: null, phase: 'lobby' })
    },
  )

  it('starts from the batch at 0 after a Snapshot recovers it', () => {
    const h = lobbyStartOnly()
    h.deliver(lobbyStart.raw[3])
    expect(h.sentOf('Resync')).toHaveLength(1)
    const recovered = truth(lobbyStart, 4)
    h.deliver({ type: 'Snapshot', seq: recovered.seq, state: recovered.state })
    h.deliver(...lobbyStart.raw.slice(4))
    expect(h.state().view).toEqual(lobbyStart.expected)
    expect(h.sentOf('Resync')).toHaveLength(1)
  })
})
