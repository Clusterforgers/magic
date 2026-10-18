// Goal: the session keeps the newest lobby roster the open connection pushed, replaces it as a whole each time, drops it when the connection goes or the game starts, sends Ready and NotReady as messages of their own, and ignores a roster it cannot read.
import { describe, expect, it } from 'vitest'
import { startSession } from '@/test-utils/harness'
import { emptyTable, lobbyMessage, withSeats } from '@/test-utils/lobbyFrames'
import { lateJoin, lobbyStart } from '@/test-utils/transcripts'

const table = emptyTable(3)

describe('the roster', () => {
  it('is empty until the server pushes one', () => {
    const h = startSession()
    expect(h.state().lobby).toBeNull()
    h.join(0, 3)
    expect(h.state().lobby).toBeNull()
  })

  it('is what the server last said, in whole: seats, names, flags and how many are needed', () => {
    const h = startSession()
    h.join(0, 3)
    h.deliver(lobbyMessage(withSeats(table, { 0: [true, false] })))
    expect(h.state().lobby).toEqual({
      seats: [
        { seat: 0, name: 'Ann', connected: true, ready: false },
        { seat: 1, name: 'Bea', connected: false, ready: false },
        { seat: 2, name: 'Cy', connected: false, ready: false },
      ],
      needed: 3,
    })
    h.deliver(lobbyMessage(withSeats(table, { 0: [true, true], 1: [true, false] })))
    expect(h.state().lobby?.needed).toBe(2)
    expect(h.state().lobby?.seats.map((seat) => [seat.connected, seat.ready])).toEqual([
      [true, true],
      [true, false],
      [false, false],
    ])
  })

  it('is replaced by each roster and not merged with the one before', () => {
    const h = startSession()
    h.join(0, 3)
    h.deliver(lobbyMessage(withSeats(table, { 0: [true, true], 1: [true, true], 2: [true, true] })))
    h.deliver(lobbyMessage(withSeats(table, { 0: [true, false] })))
    expect(h.state().lobby?.seats.map((seat) => seat.ready)).toEqual([false, false, false])
  })

  it('is dropped when the connection goes, since it was the open connection that was told', () => {
    const h = startSession()
    h.join(0, 3)
    h.deliver(lobbyMessage(withSeats(table, { 0: [true, true] })))
    h.net.last.fail()
    expect(h.state().lobby).toBeNull()
    h.timers.advance(250)
    h.open()
    expect(h.state().lobby).toBeNull()
    h.deliver({ ...lobbyMessage(table) })
    expect(h.state().lobby).not.toBeNull()
  })

  it('is dropped when the connection is closed on purpose', () => {
    const h = startSession()
    h.join(0, 3)
    h.deliver(lobbyMessage(table))
    h.session.disconnect()
    expect(h.state().lobby).toBeNull()
  })

  it('is dropped by the opening events of a game that starts, and by a snapshot of one that runs', () => {
    const lobby = startSession()
    lobby.join(0, 2)
    lobby.deliver(lobbyMessage(emptyTable(2)))
    expect(lobby.state().phase).toBe('lobby')
    lobby.deliver(lobbyStart.raw[1])
    expect(lobby.state().phase).toBe('playing')
    expect(lobby.state().lobby).toBeNull()

    const late = startSession()
    late.join(0, 2)
    late.deliver(lobbyMessage(emptyTable(2)), lateJoin.raw[1])
    expect(late.state().lobby).toBeNull()
  })

  it('does not change the phase: a lobby is still a lobby when a roster arrives', () => {
    const h = startSession()
    h.join(0, 2)
    h.deliver(lobbyMessage(emptyTable(2)))
    expect(h.state().phase).toBe('lobby')
    expect(h.state().view).toBeNull()
    expect(h.state().seq).toBeNull()
  })

  it.each([
    ['seats out of order', { type: 'Lobby', needed: 1, seats: [lobbyMessage(table).seats[1]] }],
    ['no seats list', { type: 'Lobby', needed: 1 }],
    [
      'a flag that is text',
      { type: 'Lobby', needed: 0, seats: [{ seat: 0, name: 'A', connected: 'y', ready: true }] },
    ],
  ])('ignores a roster with %s, and says so', (_name, frame) => {
    const h = startSession()
    h.join(0, 3)
    h.deliver(lobbyMessage(table))
    const before = h.state().lobby
    h.deliver(frame)
    expect(h.state().lobby).toBe(before)
    expect(h.state().notice).toMatch(/could not read/i)
  })
})

describe('Ready and NotReady', () => {
  it('are separate messages, one per call, and the roster is not touched by sending them', () => {
    const h = startSession()
    h.join(0, 3)
    h.deliver(lobbyMessage(table))
    const before = h.state().lobby
    expect(h.session.ready()).toBe(true)
    expect(h.session.notReady()).toBe(true)
    expect(h.session.ready()).toBe(true)
    expect(h.sentTypes()).toEqual(['Join', 'Ready', 'NotReady', 'Ready'])
    expect(h.state().lobby).toBe(before)
  })

  it('send nothing and say so while the socket is not open', () => {
    const h = startSession()
    expect(h.session.notReady()).toBe(false)
    h.join(0, 3)
    h.net.last.fail()
    expect(h.session.ready()).toBe(false)
    expect(h.session.notReady()).toBe(false)
    expect(h.sentTypes()).toEqual(['Join'])
  })
})
