// Goal: the line under the banner names exactly the seats that are away among those the game waits for, then any other seat away on a clock, with whole seconds left; a seat that left the game or is connected is never "away".
import { describe, expect, it } from 'vitest'
import { initialPlayState } from '@/play/store'
import type { PlayState } from '@/play/store'
import type { PlayerView, StateView } from '@/protocol/generated'
import { awaySeats, awayText, isAway, untimed } from './presenceText'
import type { AwaySeat } from './presenceText'
import type { Waiting } from './turnText'

const NAMES = ['Ann', 'Bo', 'Cy', 'Di']
const name = (seat: number): string => NAMES[seat] ?? `Seat ${seat}`

/** A table of four whose seats have the given (connected, lost) flags and away clocks. */
function table(seats: [connected: boolean, lost: boolean][], away: PlayState['away'] = {}): PlayState {
  const players = seats.map(
    ([connected, lost], seat) => ({ seat, connected, lost: lost ? 'Conceded' : null }) as unknown as PlayerView,
  )
  return { ...initialPlayState(), view: { players } as unknown as StateView, away }
}

const others = (...seats: number[]): Waiting => ({ kind: 'others', seats })

describe('isAway', () => {
  it.each([
    { connected: true, lost: false, away: false },
    { connected: false, lost: false, away: true },
    { connected: false, lost: true, away: false },
    { connected: true, lost: true, away: false },
  ])('connected $connected, lost $lost: away $away', ({ connected, lost, away }) => {
    expect(isAway(table([[connected, lost]]), 0)).toBe(away)
  })
})

describe('awaySeats', () => {
  const seats: [boolean, boolean][] = [
    [true, false],
    [false, false],
    [false, false],
    [false, true],
  ]
  it.each([
    { name: 'the waited-on seat that is away', waiting: others(1), away: {}, expected: [{ seat: 1, leftMs: null }] },
    { name: 'nobody when the waited-on seat is here', waiting: others(0), away: {}, expected: [] },
    {
      name: 'a clocked seat after the waited-on ones',
      waiting: others(1),
      away: { 2: 7_000 },
      expected: [
        { seat: 1, leftMs: null },
        { seat: 2, leftMs: 7_000 },
      ],
    },
    { name: 'never a seat that left the game', waiting: others(3), away: { 3: 5_000 }, expected: [] },
    {
      name: 'a clocked seat while the game waits for nobody',
      waiting: { kind: 'idle', label: 'Waiting for the game' } as Waiting,
      away: { 2: 3_000 },
      expected: [{ seat: 2, leftMs: 3_000 }],
    },
  ])('$name', ({ waiting, away, expected }) => {
    expect(awaySeats(table(seats, away), waiting)).toEqual(expected)
  })
})

describe('awayText', () => {
  it.each<{ seats: AwaySeat[]; text: string }>([
    { seats: [], text: '' },
    { seats: [{ seat: 1, leftMs: null }], text: 'Bo is away' },
    { seats: [{ seat: 1, leftMs: 11_200 }], text: 'Bo is away: the game acts for them in 12 s' },
    { seats: [{ seat: 1, leftMs: 0 }], text: 'Bo is away: the game acts for them in 0 s' },
    {
      seats: [
        { seat: 1, leftMs: 4_000 },
        { seat: 2, leftMs: null },
      ],
      text: 'Bo is away: the game acts for them in 4 s; Cy is away',
    },
  ])('$text', ({ seats, text }) => {
    expect(awayText(seats, name)).toBe(text)
  })

  it('speaks no seconds once untimed', () => {
    expect(awayText(untimed([{ seat: 2, leftMs: 9_000 }]), name)).toBe('Cy is away')
  })
})
