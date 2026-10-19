// Goal: the words for a finished game are right for every way a seat can lose and every point of view: the winner, the loser, a draw, and a game seen without a view; and the kind of ending, a win, a loss, a draw or a concession, is read from the losses of the seats that are out.
import { describe, expect, it } from 'vitest'
import type { GameResult, LossReason, PlayerView } from '@/protocol/generated'
import { clone } from '@/test-utils/batches'
import { lateJoin } from '@/test-utils/transcripts'
import { describeLoss, describeResult, endKind, seatOutcomes } from './outcome'
import type { EndKind } from './outcome'

const name = (seat: number): string => ['Ana', 'Bo', 'Cy'][seat] ?? `Seat ${seat + 1}`

const base = lateJoin.expected.players[0]
if (base === undefined) throw new Error('the fixture has no players')

/** Players from the fixture with the given losses, by seat. */
const players = (...lost: (LossReason | null)[]): PlayerView[] =>
  lost.map((reason, seat) => ({ ...clone(base), seat, lost: reason, life: reason === null ? 20 : 0 }))

describe('describeLoss', () => {
  it.each<[LossReason, string]>([
    ['ZeroLife', 'ran out of life'],
    ['EmptyLibrary', 'could not draw from an empty library'],
    ['Conceded', 'conceded'],
    [{ Format: 'commander damage' }, 'lost: commander damage'],
  ])('puts %j as "%s"', (reason, words) => {
    expect(describeLoss(reason)).toBe(words)
  })
})

describe('describeResult', () => {
  const cases: [string, GameResult, number | null, (LossReason | null)[], string, string | null][] = [
    ['the winner, the other conceded', { Winner: 0 }, 0, [null, 'Conceded'], 'You won', 'Bo conceded.'],
    [
      'the winner, the other ran out of life',
      { Winner: 1 },
      1,
      ['ZeroLife', null],
      'You won',
      'Ana ran out of life.',
    ],
    ['the loser who conceded', { Winner: 1 }, 0, ['Conceded', null], 'You lost', 'Bo won. You conceded.'],
    [
      'the loser on the board',
      { Winner: 1 },
      0,
      ['ZeroLife', null],
      'You lost',
      'Bo won. You ran out of life.',
    ],
    [
      'the winner of three, two others lost',
      { Winner: 0 },
      0,
      [null, 'Conceded', 'ZeroLife'],
      'You won',
      'Bo conceded and Cy ran out of life.',
    ],
    [
      'a loser of three',
      { Winner: 2 },
      0,
      ['EmptyLibrary', 'Conceded', null],
      'You lost',
      'Cy won. You could not draw from an empty library and Bo conceded.',
    ],
    ['a seat still in when someone else won', { Winner: 1 }, 0, [null, null], 'Bo won', null],
    ['a draw with nobody to blame', 'Draw', 0, [null, null], 'The game is a draw', 'Nobody won.'],
    [
      'a draw both lost',
      'Draw',
      0,
      ['ZeroLife', 'ZeroLife'],
      'The game is a draw',
      'You ran out of life and Bo ran out of life.',
    ],
    ['a game seen without a view', { Winner: 1 }, 0, [], 'Bo won', null],
    ['a game seen by nobody', { Winner: 1 }, null, [null, 'Conceded'], 'Bo won', 'Bo conceded.'],
    ['a draw without a view', 'Draw', 0, [], 'The game is a draw', 'Nobody won.'],
  ]
  it.each(cases)('%s', (_what, result, me, lost, headline, detail) => {
    expect(describeResult(result, me, name, players(...lost))).toEqual({ headline, detail })
  })
})

describe('endKind', () => {
  const cases: [string, GameResult, number | null, (LossReason | null)[], EndKind][] = [
    ['the viewer won on the board', { Winner: 0 }, 0, [null, 'ZeroLife'], 'won'],
    ['the viewer lost on the board', { Winner: 1 }, 0, ['EmptyLibrary', null], 'lost'],
    ['the viewer watches a game another seat won', { Winner: 1 }, 2, [null, null, 'ZeroLife'], 'lost'],
    ['a draw', 'Draw', 0, [null, null], 'drawn'],
    ['the other seat conceded', { Winner: 0 }, 0, [null, 'Conceded'], 'conceded'],
    ['the viewer conceded', { Winner: 1 }, 0, ['Conceded', null], 'conceded'],
    [
      'a seat left a game of three and the third ran out of life',
      { Winner: 0 },
      0,
      [null, 'Conceded', 'ZeroLife'],
      'conceded',
    ],
    ['three seats that all lost on the board', { Winner: 0 }, 0, [null, 'ZeroLife', 'EmptyLibrary'], 'won'],
    [
      'a concession while a seat is still in, which is no end by concession',
      { Winner: 0 },
      0,
      [null, 'Conceded', null],
      'won',
    ],
    ['the result is all that is known', { Winner: 1 }, 0, [], 'lost'],
    ['the result is all that is known and it is a win', { Winner: 0 }, 0, [], 'won'],
  ]

  it.each(cases)('%s is %s', (_what, result, me, lost, kind) => {
    expect(endKind(result, me, players(...lost))).toBe(kind)
  })
})

describe('seatOutcomes', () => {
  it('says who won, who lost and how, in seat order, and which seat is the viewer', () => {
    expect(seatOutcomes({ Winner: 0 }, 0, name, players(null, 'Conceded', 'ZeroLife'))).toEqual([
      { seat: 0, name: 'Ana', you: true, status: 'Winner', life: 20 },
      { seat: 1, name: 'Bo', you: false, status: 'Lost: conceded', life: 0 },
      { seat: 2, name: 'Cy', you: false, status: 'Lost: ran out of life', life: 0 },
    ])
  })

  it('marks every seat that did not lose as Draw when the game is drawn', () => {
    expect(seatOutcomes('Draw', 1, name, players(null, null)).map((entry) => entry.status)).toEqual([
      'Draw',
      'Draw',
    ])
  })

  it('has nothing to list without a view', () => {
    expect(seatOutcomes({ Winner: 0 }, 0, name, [])).toEqual([])
  })
})
