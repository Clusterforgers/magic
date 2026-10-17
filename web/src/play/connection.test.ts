// Goal: the session mirrors the socket's status, keeps the game across a reconnect, forgets what belongs to a dead connection, and stops retrying once the game is over.
import { describe, expect, it } from 'vitest'
import { PROTOCOL_VERSION } from '@/protocol/generated'
import { asRecord, must } from '@/test-utils/batches'
import { priorityDecision, startSession } from '@/test-utils/harness'
import { lateJoin, lobbyStart } from '@/test-utils/transcripts'

const at = (count: number): unknown[] => lateJoin.raw.slice(0, count)

describe('the connection as the session reports it', () => {
  it('goes from connecting to open to reconnecting and back, counting failed attempts', () => {
    const h = startSession()
    const seen: string[] = []
    h.session.store.subscribe((state, before) => {
      if (state.status !== before.status || state.attempt !== before.attempt)
        seen.push(`${state.status}:${state.attempt}`)
    })
    h.session.connect()
    h.net.last.fail()
    h.timers.advance(250)
    h.net.last.fail()
    h.timers.advance(500)
    h.open()
    h.deliver(lateJoin.raw[0])
    h.net.last.serverClose(1001)
    expect(seen).toEqual(['connecting:0', 'reconnecting:1', 'reconnecting:2', 'open:0', 'reconnecting:1'])
    expect(h.state().seat).toBe(0)
  })

  it('opens with the game and name it was given, at the seat address on the page origin', () => {
    const h = startSession({ game: 'g-77', token: 'secret/1', name: 'Bea', origin: 'https://play.example' })
    h.session.connect()
    h.open()
    expect(h.net.last.url).toBe('wss://play.example/ws/g-77/secret%2F1')
    expect(h.sent()).toEqual([
      { type: 'Join', protocol_version: PROTOCOL_VERSION, game: 'g-77', name: 'Bea' },
    ])
  })

  it('survives the connect, disconnect, connect of a double-run effect with one live socket', () => {
    const h = startSession()
    h.session.connect()
    h.session.disconnect()
    h.session.connect()
    expect(h.net.sockets).toHaveLength(2)
    expect(h.net.sockets[0]?.closedByClient).toEqual({ code: 1000, reason: 'client closing' })
    h.open()
    h.deliver(lateJoin.raw[0])
    expect(h.state()).toMatchObject({ status: 'open', seat: 0 })
    expect(h.sentTypes()).toEqual(['Join'])
    expect(h.net.sockets[0]?.sentTypes).toEqual([])
  })

  it('goes idle on disconnect, keeps the game and has no decision left to answer', () => {
    const h = startSession()
    h.join()
    h.deliver(...lateJoin.raw.slice(1, 5))
    const view = h.state().view
    expect(h.state().pending).not.toBeNull()
    h.session.disconnect()
    expect(h.state()).toMatchObject({ status: 'idle', pending: null })
    expect(h.state().view).toBe(view)
    expect(h.timers.pending()).toEqual([])
  })

  it('keeps two sessions apart', () => {
    const one = startSession()
    const two = startSession()
    one.join()
    two.join(1, 3)
    one.deliver(priorityDecision(5), { type: 'Chat', from: 1, text: 'only for one' })
    expect(one.state()).toMatchObject({ seat: 0, seatCount: 2, pending: { id: 5 } })
    expect(two.state()).toMatchObject({ seat: 1, seatCount: 3, pending: null, chat: [] })
  })
})

describe('across a reconnect', () => {
  function midGame() {
    const h = startSession()
    h.join()
    h.deliver(...lateJoin.raw.slice(1, 5))
    return h
  }

  it('keeps showing the game, drops the decision, and takes the Snapshot without redrawing what is unchanged', () => {
    const h = midGame()
    const before = h.state()
    h.net.last.fail()
    expect(h.state()).toMatchObject({ status: 'reconnecting', pending: null })
    expect(h.state().view).toBe(before.view)
    expect(h.state().log).toBe(before.log)
    h.timers.advance(250)
    h.open()
    // The snapshot the server sends on rejoin describes the same moment as the view the client already has.
    const resent = asRecord(lateJoin.raw[1])
    h.deliver(lateJoin.raw[0], { ...resent, seq: before.seq, state: before.view })
    expect(h.state().status).toBe('open')
    expect(h.state().view).toBe(before.view)
    expect(h.state().objectsById).toBe(before.objectsById)
    expect(h.state().seq).toBe(before.seq)
  })

  it('forgets a Resync that was waiting for its Snapshot', () => {
    const h = midGame()
    const seq = must(h.state().seq, 'seq')
    h.deliver({ ...asRecord(lateJoin.raw[5]), from_seq: seq + 1 })
    expect(h.sentOf('Resync')).toHaveLength(1)
    h.net.last.fail()
    h.timers.advance(250)
    h.open()
    // After the new Snapshot the very next batch in order is applied, not held back by the dead connection's gap.
    h.deliver(...at(2), ...lateJoin.raw.slice(2, 5))
    expect(h.state().seq).toBe(seq)
    h.deliver(lateJoin.raw[5])
    expect(h.state().seq).toBe(seq + 5)
    expect(h.sentOf('Resync')).toHaveLength(0)
  })

  it('takes a seat back after it was replaced, with the old game still on screen', () => {
    const h = midGame()
    const view = h.state().view
    h.net.last.serverClose(4000)
    expect(h.state().status).toBe('replaced')
    expect(h.state().view).toBe(view)
    h.session.connect()
    expect(h.state()).toMatchObject({ status: 'connecting', attempt: 0, notice: null })
    expect(h.net.sockets).toHaveLength(2)
  })
})

describe('when the game is over', () => {
  it.each([1001, 1006, 1013])(
    'a lost connection (code %i) is final: no retry, no notice, the result stays',
    (code) => {
      const h = startSession()
      h.join()
      h.deliver(...lateJoin.raw.slice(1))
      expect(h.state().phase).toBe('over')
      h.net.last.serverClose(code)
      expect(h.state()).toMatchObject({
        status: 'stopped',
        notice: null,
        phase: 'over',
        result: { Winner: 0 },
      })
      expect(h.state().view).toEqual(lateJoin.expected)
      h.timers.advance(10 * 60_000)
      expect(h.net.sockets).toHaveLength(1)
    },
  )

  it('still reconnects when the connection is lost before the game ends', () => {
    const h = startSession()
    h.join()
    h.deliver(...lateJoin.raw.slice(1, 10))
    h.net.last.serverClose(1006)
    expect(h.state().status).toBe('reconnecting')
  })

  it('lets chat through after the game is over, while the socket is up', () => {
    const h = startSession()
    h.join()
    h.deliver(...lateJoin.raw.slice(1))
    expect(h.session.chat('gg')).toBe(true)
    expect(h.sentOf('Chat')).toEqual([{ type: 'Chat', text: 'gg' }])
  })
})

describe('objects by id', () => {
  it('keeps the very same object as long as no batch lists it', () => {
    const h = startSession()
    h.session.connect()
    h.open()
    let previous = h.state().objectsById
    let steps = 0
    let kept = 0
    for (const raw of lobbyStart.raw) {
      h.deliver(raw)
      const message = asRecord(raw)
      const now = h.state().objectsById
      if (message.type === 'Events') {
        const listed = new Set(
          (Array.isArray(message.objects) ? message.objects : []).map((o: unknown) => asRecord(o).id),
        )
        for (const [id, object] of Object.entries(now)) {
          if (!listed.has(Number(id)) && previous[Number(id)] !== undefined) {
            expect(object).toBe(previous[Number(id)])
            kept += 1
          }
        }
        steps += 1
      }
      previous = now
    }
    expect(steps).toBe(40)
    expect(kept).toBeGreaterThan(0)
  })
})
