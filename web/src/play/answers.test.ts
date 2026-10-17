// Goal: an Answer is sent once per decision id on a connection, never after the pending decision is cleared and never while the socket is down; every other action is sent only on an open socket.
import { describe, expect, it } from 'vitest'
import { pass, priorityDecision, startSession } from '@/test-utils/harness'
import { lateJoin } from '@/test-utils/transcripts'
import type { Session } from './session'

const snapshot = lateJoin.raw[1]

function pending(id = 7) {
  const h = startSession()
  h.join()
  h.deliver(priorityDecision(id))
  return h
}

describe('answering', () => {
  it('sends the answer under the pending decision id and clears the decision', () => {
    const h = pending(7)
    expect(h.state().pending?.id).toBe(7)
    expect(h.session.answer(pass)).toBe(true)
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 7, answer: pass }])
    expect(h.state().pending).toBeNull()
  })

  it('passes the answer through untouched', () => {
    const h = pending(7)
    h.session.answer({ Discard: [3, 4] })
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 7, answer: { Discard: [3, 4] } }])
  })

  it('sends nothing for a second answer to the same decision, however often it is tried', () => {
    const h = pending(7)
    expect(h.session.answer(pass)).toBe(true)
    for (let attempt = 0; attempt < 5; attempt += 1) expect(h.session.answer(pass)).toBe(false)
    expect(h.answeredIds()).toEqual([7])
  })

  it('answers each decision once when they come one after another', () => {
    const h = pending(7)
    h.session.answer(pass)
    h.deliver(priorityDecision(8))
    h.session.answer(pass)
    h.deliver(priorityDecision(12))
    h.session.answer(pass)
    h.session.answer(pass)
    expect(h.answeredIds()).toEqual([7, 8, 12])
  })

  it('answers only the newer decision when it replaces one nobody answered', () => {
    const h = pending(7)
    h.deliver(priorityDecision(8))
    expect(h.state().pending?.id).toBe(8)
    h.session.answer(pass)
    expect(h.answeredIds()).toEqual([8])
  })

  it('ignores a decision that is delivered again after it was answered', () => {
    const h = pending(7)
    h.session.answer(pass)
    h.deliver(priorityDecision(7))
    expect(h.state().pending).toBeNull()
    expect(h.session.answer(pass)).toBe(false)
    expect(h.answeredIds()).toEqual([7])
  })

  it('has nothing to answer before a decision arrives or after the game ends', () => {
    const h = startSession()
    expect(h.session.answer(pass)).toBe(false)
    h.join()
    expect(h.session.answer(pass)).toBe(false)
    h.deliver(priorityDecision(7), { type: 'GameOver', result: 'Draw' })
    expect(h.state().pending).toBeNull()
    expect(h.session.answer(pass)).toBe(false)
    expect(h.answeredIds()).toEqual([])
  })

  it('may answer the same id again once the server resyncs after refusing the answer', () => {
    const h = pending(7)
    h.session.answer(pass)
    h.deliver({ type: 'Error', code: 'IllegalAnswer', message: 'that is not a legal land' })
    expect(h.sentOf('Resync')).toHaveLength(1)
    // The server's reply to the Resync: the snapshot, then the decision it is still waiting on.
    h.deliver(snapshot, priorityDecision(7))
    expect(h.state().pending?.id).toBe(7)
    expect(h.session.answer({ Discard: [] })).toBe(true)
    expect(h.answeredIds()).toEqual([7, 7])
  })
})

describe('answering while the socket is down', () => {
  const ways: [string, (h: ReturnType<typeof pending>) => void][] = [
    ['the line drops', (h) => h.net.last.fail()],
    ['the server goes away', (h) => h.net.last.serverClose(1001)],
    ['the seat is taken by another window', (h) => h.net.last.serverClose(4000)],
    [
      'the server stops the socket for a broken rule',
      (h) => h.net.last.serverClose(1008, 'rate limit exceeded'),
    ],
    ['the page disconnects', (h) => h.session.disconnect()],
  ]

  it.each(ways)('after %s there is no decision and no answer goes out', (_what, lose) => {
    const h = pending(7)
    const socket = h.net.last
    lose(h)
    expect(h.state().pending).toBeNull()
    expect(h.session.answer(pass)).toBe(false)
    expect(socket.sentTypes).toEqual(['Join'])
    h.timers.advance(120_000)
    for (const later of h.net.sockets.slice(1)) expect(later.sentTypes).not.toContain('Answer')
  })

  it('offers the same decision id again on the new connection, because the first answer may have been lost', () => {
    const h = pending(7)
    expect(h.session.answer(pass)).toBe(true)
    h.net.last.fail()
    h.timers.advance(250)
    h.open()
    h.deliver(lateJoin.raw[0], snapshot, priorityDecision(7))
    expect(h.state().pending?.id).toBe(7)
    expect(h.session.answer(pass)).toBe(true)
    expect(h.net.sockets).toHaveLength(2)
    expect(h.answeredIds()).toEqual([7])
    expect(h.net.sockets[0]?.sentTypes).toEqual(['Join', 'Answer'])
  })

  it('has nothing to answer on the new connection until its decision arrives', () => {
    const h = pending(7)
    h.net.last.fail()
    h.timers.advance(250)
    h.open()
    h.deliver(lateJoin.raw[0], snapshot)
    expect(h.session.answer(pass)).toBe(false)
    expect(h.answeredIds()).toEqual([])
  })
})

describe('the other actions', () => {
  const actions: [string, (session: Session) => boolean, unknown][] = [
    ['ready', (s) => s.ready(), { type: 'Ready' }],
    ['concede', (s) => s.concede(), { type: 'Concede' }],
    ['chat', (s) => s.chat('  good luck  '), { type: 'Chat', text: 'good luck' }],
    ['resync', (s) => s.resync(), { type: 'Resync' }],
  ]

  it.each(actions)('%s sends its message on an open socket only', (_name, act, message) => {
    const h = startSession()
    expect(act(h.session)).toBe(false)
    h.session.connect()
    expect(act(h.session)).toBe(false)
    h.open()
    h.deliver(lateJoin.raw[0])
    expect(act(h.session)).toBe(true)
    expect(h.sent()).toEqual([expect.objectContaining({ type: 'Join' }), message])
    h.net.last.fail()
    expect(act(h.session)).toBe(false)
    expect(h.sent()).toHaveLength(2)
  })
})
