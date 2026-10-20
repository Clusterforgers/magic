// Goal: the lobby rows come from the server's roster when there is one and from the REST lookup otherwise, the summary line and the spoken announcement say the right thing for every mix of present, away, ready and waiting seats at tables of 2 to 12, and a seat's state follows from its two flags.
import { describe, expect, it } from 'vitest'
import type { LobbyRoster } from '@/play/store'
import type { GameInfo } from '@/protocol/generated'
import { emptyTable, lobbyMessage, SEAT_DECK, withSeats } from '@/test-utils/lobbyFrames'
import { announcement, lobbyRows, removable, seatState, summaryText } from './lobbyModel'
import type { LobbyRow } from './lobbyModel'

const NAMES = ['Ann', 'Bea', 'Cy', 'Di', 'Ed', 'Flo', 'Gus', 'Hal', 'Ivy', 'Jo', 'Kit', 'Lu']

/**
 * Rows from a picture: one letter per seat, `R` ready and here, `r` ready but away, `p` here and not ready yet, `a`
 * not here. `you` is the seat number of this page's seat, if any.
 */
function picture(seats: string, you = -1): LobbyRow[] {
  return [...seats].map((letter, seat) => ({
    seat,
    name: NAMES[seat] ?? `Seat ${seat + 1}`,
    named: true,
    you: seat === you,
    connected: letter === 'R' || letter === 'p',
    ready: letter === 'R' || letter === 'r',
    deck: SEAT_DECK,
  }))
}

const roster = (seats: Parameters<typeof withSeats>[1], count = 3): LobbyRoster => {
  const { seats: listed, needed } = lobbyMessage(withSeats(emptyTable(count), seats))
  return { seats: listed, needed }
}

const info = (over: Partial<GameInfo> = {}): GameInfo => ({
  started: false,
  seats: [
    { seat: 0, name: 'Ann' },
    { seat: 1, name: 'Bea' },
    { seat: 2, name: 'Cy' },
  ],
  connected: [],
  ready: [],
  result: null,
  visibility: 'invite',
  ...over,
})

describe('lobbyRows', () => {
  it('lists the seats of the roster with its names and flags, and takes this page for the seat it plays', () => {
    const pushed = roster({ 0: [true, true], 1: [true, false], 2: [false, true] })
    expect(lobbyRows({ roster: pushed, info: null, open: true, seat: 1, seatCount: 3 })).toEqual([
      { seat: 0, name: 'Ann', named: true, you: false, connected: true, ready: true, deck: SEAT_DECK },
      { seat: 1, name: 'Bea', named: true, you: true, connected: true, ready: false, deck: SEAT_DECK },
      { seat: 2, name: 'Cy', named: true, you: false, connected: false, ready: true, deck: SEAT_DECK },
    ])
  })

  it('lets the roster win over the lookup for everything it knows', () => {
    const stale = info({ connected: [0, 1, 2], ready: [0, 1, 2] })
    const pushed = roster({ 1: [true, false] })
    const rows = lobbyRows({ roster: pushed, info: stale, open: true, seat: 0, seatCount: 3 })
    expect(rows.map((row) => [row.connected, row.ready])).toEqual([
      [true, false], // this page's own socket is open
      [true, false],
      [false, false],
    ])
  })

  it('reads a seat the roster calls open as open, though the lookup still has the name of the player just removed', () => {
    const { seats, needed } = lobbyMessage([
      ['Ann', true, false],
      ['Bea', true, false],
      ['', false, false],
    ])
    const rows = lobbyRows({ roster: { seats, needed }, info: info(), open: true, seat: 0, seatCount: 3 })
    expect(rows.map((row) => [row.name, row.named])).toEqual([
      ['Ann', true],
      ['Bea', true],
      ['Seat 3', false],
    ])
  })

  it('uses the lookup when there is no roster: names, who is here and who is ready', () => {
    const rows = lobbyRows({
      roster: null,
      info: info({ connected: [1], ready: [1, 2] }),
      open: true,
      seat: 0,
      seatCount: 3,
    })
    expect(rows.map((row) => [row.name, row.connected, row.ready])).toEqual([
      ['Ann', true, false],
      ['Bea', true, true],
      ['Cy', false, true],
    ])
  })

  it('knows this page is away when its socket is not open, whatever the lookup says', () => {
    const rows = lobbyRows({
      roster: null,
      info: info({ connected: [0, 1] }),
      open: false,
      seat: 0,
      seatCount: 3,
    })
    expect(rows.map((row) => row.connected)).toEqual([false, true, false])
  })

  it('shows "Seat N" for every seat of the table before anything has named them', () => {
    const rows = lobbyRows({ roster: null, info: null, open: true, seat: 2, seatCount: 4 })
    expect(rows.map((row) => [row.name, row.named, row.connected, row.ready])).toEqual([
      ['Seat 1', false, false, false],
      ['Seat 2', false, false, false],
      ['Seat 3', false, true, false],
      ['Seat 4', false, false, false],
    ])
  })

  it('has the roster decide the number of seats, then the welcome, then the lookup', () => {
    const count = (sources: Partial<Parameters<typeof lobbyRows>[0]>): number =>
      lobbyRows({ roster: null, info: null, open: true, seat: null, seatCount: null, ...sources }).length
    expect(count({})).toBe(0)
    expect(count({ info: info() })).toBe(3)
    expect(count({ info: info(), seatCount: 5 })).toBe(5)
    expect(count({ info: info(), seatCount: 5, roster: roster({}, 2) })).toBe(2)
  })

  it('names a seat the lookup does not list "Seat N"', () => {
    const partial = info({ seats: [{ seat: 0, name: 'Ann' }] })
    const rows = lobbyRows({ roster: null, info: partial, open: true, seat: 0, seatCount: 2 })
    expect(rows.map((row) => [row.name, row.named])).toEqual([
      ['Ann', true],
      ['Seat 2', false],
    ])
  })

  it('marks exactly one seat as this page, or none before it is welcomed', () => {
    const yous = (seat: number | null) =>
      lobbyRows({ roster: roster({}), info: null, open: true, seat, seatCount: 3 }).filter((row) => row.you)
    expect(yous(2).map((row) => row.seat)).toEqual([2])
    expect(yous(null)).toEqual([])
    expect(yous(7)).toEqual([])
  })
})

describe('summaryText', () => {
  it.each([
    ['nobody there yet', 'aa', -1, '0 of 2 ready, waiting for Ann and Bea to join'],
    ['everybody there and nobody ready', 'pp', -1, '0 of 2 ready, waiting for Ann and Bea to press Ready'],
    ['one ready, the other not joined', 'Ra', -1, '1 of 2 ready, waiting for Bea to join'],
    ['one ready, the other there', 'Rp', -1, '1 of 2 ready, waiting for Bea to press Ready'],
    ['the example of the brief', 'RRa', -1, '2 of 3 ready, waiting for Cy to join'],
    ['this seat is the one waited on', 'pR', 0, '1 of 2 ready, waiting for you to press Ready'],
    [
      'this seat among others',
      'ppa',
      1,
      '0 of 3 ready, waiting for Ann and you to press Ready and Cy to join',
    ],
    ['a ready seat that is away is not waited on', 'Rra', -1, '2 of 3 ready, waiting for Cy to join'],
    ['only away seats are ready', 'rrp', -1, '2 of 3 ready, waiting for Cy to press Ready'],
    ['three names are listed', 'Raaa', -1, '1 of 4 ready, waiting for Bea, Cy and Di to join'],
    ['four are only counted', 'Raaaa', -1, '1 of 5 ready, waiting for 4 players to join'],
    [
      'six seats, a mix',
      'RRppaa',
      -1,
      '2 of 6 ready, waiting for Cy and Di to press Ready and Ed and Flo to join',
    ],
    ['twelve seats, nobody ready', 'a'.repeat(12), -1, '0 of 12 ready, waiting for 12 players to join'],
    ['everybody ready', 'RR', -1, 'All 2 seats are ready, starting the game'],
    ['everybody ready, one away', 'RRr', -1, 'All 3 seats are ready, starting the game'],
  ])('%s', (_name, seats, you, expected) => {
    expect(summaryText(picture(seats, you))).toBe(expected)
  })

  it('says nothing for a table with no seats', () => {
    expect(summaryText([])).toBe('')
  })

  it('prints a name as it is, long and strange ones too', () => {
    const rows = picture('Rpa')
    const odd = '<b>' + 'x'.repeat(29)
    const named = rows.map((row) => (row.seat === 1 ? { ...row, name: odd } : row))
    expect(summaryText(named)).toBe(`1 of 3 ready, waiting for ${odd} to press Ready and Cy to join`)
  })
})

describe('announcement', () => {
  const say = (before: string, after: string, you = -1): string | null =>
    announcement(picture(before, you), picture(after, you))

  it.each([
    ['someone joins', 'aa', 'pa', 'Ann joined. 0 of 2 ready, waiting for Ann to press Ready and Bea to join'],
    ['someone leaves', 'pp', 'pa', 'Bea left. 0 of 2 ready, waiting for Ann to press Ready and Bea to join'],
    ['someone presses Ready', 'pp', 'Rp', 'Ann is ready. 1 of 2 ready, waiting for Bea to press Ready'],
    [
      'someone takes Ready back',
      'Rp',
      'pp',
      'Ann is not ready. 0 of 2 ready, waiting for Ann and Bea to press Ready',
    ],
    ['a ready seat goes away', 'RR', 'Rr', 'Bea left. All 2 seats are ready, starting the game'],
    ['a ready seat comes back', 'Rr', 'RR', 'Bea joined. All 2 seats are ready, starting the game'],
    [
      'a seat joins and is ready at once',
      'Ra',
      'RR',
      'Bea joined and is ready. All 2 seats are ready, starting the game',
    ],
    [
      'a seat leaves and takes Ready back',
      'RR',
      'aR',
      'Ann left and is not ready. 1 of 2 ready, waiting for Ann to join',
    ],
    [
      'two seats at once',
      'aa',
      'Rp',
      'Ann joined and is ready, Bea joined. 1 of 2 ready, waiting for Bea to press Ready',
    ],
  ])('%s', (_name, before, after, expected) => {
    expect(say(before, after)).toBe(expected)
  })

  it('speaks of this seat as "you" and leaves its own connection to the connection banner', () => {
    expect(say('pp', 'Rp', 0)).toBe('You are ready. 1 of 2 ready, waiting for Bea to press Ready')
    expect(say('Rp', 'pp', 0)).toBe('You are not ready. 0 of 2 ready, waiting for you and Bea to press Ready')
    expect(say('ap', 'pp', 0)).toBeNull()
    expect(say('pp', 'ap', 0)).toBeNull()
  })

  it('is null when nothing a person would notice changed', () => {
    expect(say('Rpa', 'Rpa')).toBeNull()
    expect(announcement([], [])).toBeNull()
    expect(announcement([], picture('RR'))).toBeNull()
  })

  it('spells out three changes and counts the rest', () => {
    expect(say('aaaa', 'pppp')).toBe(
      'Ann joined, Bea joined, Cy joined, 1 more change. 0 of 4 ready, waiting for 4 players to press Ready',
    )
    expect(say('aaaaa', 'ppppp')).toContain('Ann joined, Bea joined, Cy joined, 2 more changes. ')
  })

  it('compares seats by number, so a table that grew or shrank still reads right', () => {
    expect(announcement(picture('pp'), picture('Rpa'))).toBe(
      'Ann is ready. 1 of 3 ready, waiting for Bea to press Ready and Cy to join',
    )
    expect(announcement(picture('Rpa'), picture('pp'))).toBe(
      'Ann is not ready. 0 of 2 ready, waiting for Ann and Bea to press Ready',
    )
  })
})

describe('seatState', () => {
  it.each([
    [true, true, 'ready'],
    [false, true, 'ready'],
    [true, false, 'waiting'],
    [false, false, 'away'],
  ] as const)('connected %s, ready %s is %s', (connected, ready, expected) => {
    expect(seatState({ connected, ready })).toBe(expected)
  })
})

describe('removable', () => {
  it.each([
    ['a player who joined', 1, true, [], true],
    ['a player in a later seat', 5, true, [0], true],
    ['the host’s own seat', 0, true, [], false],
    ['an open seat', 2, false, [], false],
    ['a seat the game came with', 1, true, [1], false],
    ['an open seat that came with the game', 3, false, [3], false],
  ] as const)('%s (seat %i, named %s, issued %j) is %s', (_what, seat, named, issued, expected) => {
    expect(removable({ seat, named }, 0, new Set(issued))).toBe(expected)
  })

  it('takes the host seat as given', () => {
    expect(removable({ seat: 2, named: true }, 2, new Set())).toBe(false)
    expect(removable({ seat: 0, named: true }, 2, new Set())).toBe(true)
  })
})
