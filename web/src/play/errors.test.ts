// Goal: every server error gets its documented reaction (a notice, a Resync or a stop), and frames the session cannot read never stop it or change the game.
import { describe, expect, it } from 'vitest'
import { MAX_FRAME_CHARS } from '@/net/parse'
import type { ConnectionStatus } from '@/net/socket'
import type { ErrorCode } from '@/protocol/generated'
import { asRecord } from '@/test-utils/batches'
import type { FakeSocket } from '@/test-utils/fakes'
import { priorityDecision, startSession } from '@/test-utils/harness'
import { lateJoin } from '@/test-utils/transcripts'

function connected() {
  const h = startSession()
  h.join()
  h.deliver(priorityDecision(7))
  return h
}

describe('server errors', () => {
  const reactions: [ErrorCode, RegExp, number, ConnectionStatus][] = [
    ['StaleDecision', /no longer pending/, 1, 'open'],
    ['IllegalAnswer', /not allowed: detail/, 1, 'open'],
    ['BadMessage', /did not accept that: detail/, 0, 'open'],
    ['Internal', /internal error/, 0, 'open'],
    ['UnknownGame', /does not exist/, 0, 'stopped'],
    ['UnsupportedVersion', /protocol versions/, 0, 'stopped'],
  ]

  it.each(reactions)(
    '%s: notice %s, %i Resync, status %s, and never a reconnect loop',
    (code, notice, resyncs, status) => {
      const h = connected()
      h.deliver({ type: 'Error', code, message: 'detail' })
      expect(h.state().notice).toMatch(notice)
      expect(h.sentOf('Resync')).toHaveLength(resyncs)
      expect(h.state().status).toBe(status)
      expect(h.timers.pending()).toEqual([])
      h.timers.advance(10 * 60_000)
      expect(h.net.sockets).toHaveLength(1)
    },
  )

  it('shows the reason of a refused manual action and keeps the pending decision', () => {
    const h = connected()
    expect(h.session.manual({ Tap: { object: 3 } })).toBe(true)
    h.deliver({ type: 'Error', code: 'IllegalAnswer', message: 'Grizzly Bears is already tapped' })
    expect(h.state().notice).toMatch(/Grizzly Bears is already tapped/)
    expect(h.state().pending).toMatchObject({ id: 7 })
  })

  it.each(['BadMessage', 'Internal'])(
    '%s repeated twenty times still sends nothing and keeps the game',
    (code) => {
      const h = connected()
      for (let n = 0; n < 20; n += 1) h.deliver({ type: 'Error', code, message: `try ${n}` })
      expect(h.sentTypes()).toEqual(['Join'])
      expect(h.state()).toMatchObject({ status: 'open', pending: { id: 7 } })
    },
  )

  it.each(['UnknownGame', 'UnsupportedVersion'])('%s stops the socket by closing it with 1000', (code) => {
    const h = connected()
    h.deliver({ type: 'Error', code, message: 'detail' })
    expect(h.net.last.closedByClient).toEqual({ code: 1000, reason: 'client closing' })
    expect(h.state().pending).toBeNull()
  })

  it.each(['StaleDecision', 'IllegalAnswer'])('%s asks again only when no Resync is outstanding', (code) => {
    const h = connected()
    h.deliver({ type: 'Error', code, message: 'detail' })
    // A gap sends its own Resync and then waits; an error must not add a second one.
    const gap = { ...asRecord(lateJoin.raw[2]), from_seq: 4_000_000 }
    h.deliver(gap)
    expect(h.sentOf('Resync')).toHaveLength(2)
    h.deliver({ type: 'Error', code, message: 'detail again' })
    expect(h.sentOf('Resync')).toHaveLength(2)
    expect(h.state().notice).toMatch(/./)
  })
})

describe('frames the session cannot read', () => {
  const garbage: [string, (socket: FakeSocket) => void][] = [
    ['broken JSON', (s) => s.receive('{"type":"Chat",')],
    ['plain text', (s) => s.receive('hello')],
    ['an unknown type', (s) => s.receive({ type: 'Surprise' })],
    ['a missing field', (s) => s.receive({ type: 'Chat', from: 1 })],
    ['a wrong type', (s) => s.receive({ type: 'Chat', from: '1', text: 'x' })],
    ['a batch whose position is text', (s) => s.receive({ ...asRecord(lateJoin.raw[2]), from_seq: '992' })],
    ['a snapshot without a state', (s) => s.receive({ type: 'Snapshot', seq: 3 })],
    ['an error with an unknown code', (s) => s.receive({ type: 'Error', code: 'Nope', message: 'x' })],
    ['oversize text', (s) => s.receive('x'.repeat(MAX_FRAME_CHARS + 1))],
    ['a binary frame', (s) => s.receiveRaw(new ArrayBuffer(16))],
  ]

  it.each(garbage)(
    '%s becomes a notice; the game, the connection and the next message are untouched',
    (_what, send) => {
      const h = startSession()
      h.join()
      h.deliver(...lateJoin.raw.slice(1, 5))
      const before = h.state()
      send(h.net.last)
      const after = h.state()
      expect(after.notice).toMatch(/could not read/)
      expect({ ...after, notice: null }).toEqual({ ...before, notice: null })
      expect(after.view).toBe(before.view)
      expect(h.sentTypes()).toEqual(['Join'])
      expect(h.timers.pending()).toEqual([])
      h.deliver({ type: 'Chat', from: 1, text: 'still here' })
      expect(h.state().chat).toEqual([{ from: 1, text: 'still here' }])
    },
  )

  it('does not throw however much garbage arrives, and keeps playing the real game afterwards', () => {
    const h = startSession()
    h.session.connect()
    h.open()
    for (const [, send] of garbage) send(h.net.last)
    h.deliver(...lateJoin.raw)
    expect(h.state().view).toEqual(lateJoin.expected)
    expect(h.sentOf('Resync')).toEqual([])
  })
})

describe('the notice', () => {
  it('is replaced by a newer one and cleared by dismissNotice', () => {
    const h = connected()
    h.deliver({ type: 'Error', code: 'BadMessage', message: 'first' })
    h.deliver({ type: 'Error', code: 'BadMessage', message: 'second' })
    expect(h.state().notice).toMatch(/second/)
    h.session.dismissNotice()
    expect(h.state().notice).toBeNull()
    h.session.dismissNotice()
    expect(h.state().notice).toBeNull()
  })

  it('says why the socket stopped, in the server words, and a plain drop says nothing', () => {
    const h = connected()
    h.net.last.fail()
    expect(h.state()).toMatchObject({ status: 'reconnecting', notice: null })
    h.timers.advance(250)
    h.open()
    h.deliver(lateJoin.raw[0])
    h.net.last.serverClose(1008, 'rate limit exceeded')
    expect(h.state().status).toBe('stopped')
    expect(h.state().notice).toMatch(/rate limit exceeded/)
  })

  it('tells a player whose seat was taken, and a connect() takes it back and clears the notice', () => {
    const h = connected()
    h.net.last.serverClose(4000)
    expect(h.state()).toMatchObject({ status: 'replaced', pending: null })
    expect(h.state().notice).toMatch(/another window/)
    h.session.connect()
    expect(h.state()).toMatchObject({ status: 'connecting', notice: null })
    expect(h.net.sockets).toHaveLength(2)
  })
})
