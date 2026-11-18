// Goal: the answer in flight (`answered`) is held from the press until the server's next word on it (a batch, a decision, a snapshot, a refusal, the game's end) or the connection's loss, whichever comes first.
import { describe, expect, it } from 'vitest'
import { pass, priorityDecision, startSession } from '@/test-utils/harness'
import { lateJoin, lobbyBatchAt, lobbyStart } from '@/test-utils/transcripts'
import type { Answer } from '@/protocol/generated'

const cast: Answer = { Priority: { CastSpell: 42 } }

/** A session at the lobby's first batch, with a decision answered by `answer`. */
function answeredBeforeBatch(answer: Answer) {
  const h = startSession()
  h.join()
  h.deliver(...lobbyStart.raw.slice(1, lobbyBatchAt), priorityDecision(7))
  h.session.answer(answer)
  return h
}

describe('the answer in flight', () => {
  it('is nothing before any answer and is the answer sent once the seat answers', () => {
    const h = startSession()
    h.join()
    h.deliver(priorityDecision(7))
    expect(h.state().answered).toBeNull()
    h.session.answer(cast)
    expect(h.state().answered).toEqual(cast)
  })

  it('stays while the server says nothing, however long', () => {
    const h = answeredBeforeBatch(pass)
    h.timers.advance(10_000)
    expect(h.state().answered).toEqual(pass)
  })

  const words: [string, (h: ReturnType<typeof answeredBeforeBatch>) => void][] = [
    ['the next batch', (h) => h.deliver(lobbyStart.raw[lobbyBatchAt])],
    ['the next decision', (h) => h.deliver(priorityDecision(8))],
    ['a snapshot', (h) => h.deliver(lateJoin.raw[1])],
    ['a refusal', (h) => h.deliver({ type: 'Error', code: 'IllegalAnswer', message: 'no' })],
    ['a stale decision', (h) => h.deliver({ type: 'Error', code: 'StaleDecision', message: 'gone' })],
    ['the end of the game', (h) => h.deliver({ type: 'GameOver', result: 'Draw' })],
    ['a dropped line', (h) => h.net.last.fail()],
    ['a disconnect', (h) => h.session.disconnect()],
  ]

  it.each(words)('goes with %s', (_word, hear) => {
    const h = answeredBeforeBatch(cast)
    expect(h.state().answered).toEqual(cast)
    hear(h)
    expect(h.state().answered).toBeNull()
  })

  it('is not changed by a chat line or a roster', () => {
    const h = answeredBeforeBatch(cast)
    h.deliver({ type: 'Chat', from: 1, text: 'hi' }, { type: 'Spectators', count: 2 })
    expect(h.state().answered).toEqual(cast)
  })
})
