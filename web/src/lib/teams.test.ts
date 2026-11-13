// Goal: the team rules the web reads alone: whose turn a seat shares (the team's in Two-Headed Giant, the player's own in Teams), a seat's move keeps every other seat where it was, the lobby offers a fixed column per team, and the start check numbers the teams without gaps before it judges them.
import { describe, expect, it } from 'vitest'
import type { Teams } from '@/protocol/generated'
import { sharesTurn, teamChoices, teamsProblem, withSeatTeam } from './teams'

const GIANT: Teams = { mode: 'two_headed_giant', seats: [0, 0, 1, 1] }
const ALTERNATE: Teams = { mode: 'teams', seats: [0, 1, 0, 1] }

describe('sharesTurn', () => {
  it.each<[string, Teams | null, number, number, boolean]>([
    ['the active player, free-for-all', null, 2, 2, true],
    ['another seat, free-for-all', null, 1, 2, false],
    ['the Two-Headed Giant teammate', GIANT, 1, 0, true],
    ['a Two-Headed Giant opponent', GIANT, 2, 0, false],
    ['the Teams teammate', ALTERNATE, 2, 0, false],
    ['the Teams active player', ALTERNATE, 0, 0, true],
  ])('%s', (_what, teams, seat, active, shares) => {
    expect(sharesTurn(teams, seat, active)).toBe(shares)
  })
})

describe('withSeatTeam', () => {
  it.each<[Teams, number, number, number[]]>([
    [GIANT, 1, 1, [0, 1, 1, 1]],
    [ALTERNATE, 3, 0, [0, 1, 0, 0]],
    [{ mode: 'teams', seats: [0, 2, 0, 2] }, 0, 1, [1, 2, 0, 2]],
  ])('moves only the seat it names in %j', (teams, seat, team, seats) => {
    expect(withSeatTeam(teams, seat, team).seats).toEqual(seats)
  })

  it('returns the same teams for no move', () => {
    expect(withSeatTeam(GIANT, 0, 0)).toBe(GIANT)
  })
})

describe('teamChoices', () => {
  it.each<[Teams, number[]]>([
    [GIANT, [0, 1]],
    [{ mode: 'two_headed_giant', seats: [0, 0, 0, 0] }, [0, 1]],
    [ALTERNATE, [0, 1]],
    [{ mode: 'teams', seats: [0, 1, 0, 1, 0, 1] }, [0, 1, 2]],
    [{ mode: 'teams', seats: [0, 0, 0, 3] }, [0, 1, 2, 3]],
  ])('offers a column per team for %j', (teams, choices) => {
    expect(teamChoices(teams)).toEqual(choices)
  })
})

describe('teamsProblem', () => {
  it.each<[Teams, string | null]>([
    [GIANT, null],
    [{ mode: 'two_headed_giant', seats: [0, 1, 1, 1] }, 'Two-Headed Giant needs two teams of two.'],
    [{ mode: 'teams', seats: [1, 1, 1, 1] }, 'A team game needs at least two teams.'],
    [{ mode: 'teams', seats: [0, 2, 0, 2] }, null],
    [{ mode: 'teams', seats: [0, 0, 0, 1] }, null],
  ])('judges %j as the start does', (teams, problem) => {
    expect(teamsProblem(teams)).toBe(problem)
  })
})
