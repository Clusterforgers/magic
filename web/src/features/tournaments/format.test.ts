// Goal: tournaments are described in plain words: formats, structures, where a tournament is, each match's result, the tiebreaker percentages, the invite link and the teams a player can still join.
import { describe, expect, it } from 'vitest'
import type { EntrantView, MatchView, TournamentView } from '@/protocol/generated'
import {
  entrantName,
  formatLabel,
  inviteLink,
  isMine,
  openTeams,
  outcomeText,
  percent,
  roundDecided,
  statusLabel,
  structureLabel,
} from './format'

const entrant = (n: number, name: string, players = [name], mine = false): EntrantView => ({
  entrant: n,
  name,
  players,
  dropped: false,
  mine,
})
const ENTRANTS = [entrant(0, 'Ann', ['Ann'], true), entrant(1, 'Bob')]
const match = (b: number | null, outcome: MatchView['outcome']): MatchView => ({
  table: 1,
  a: 0,
  b,
  game: null,
  outcome,
  corrected: false,
})

describe('tournament words', () => {
  it('names formats and structures', () => {
    expect(formatLabel('commander_1v1')).toBe('Commander 1v1')
    expect(formatLabel('commander_2v2')).toBe('Commander 2v2')
    expect(structureLabel('swiss')).toBe('Swiss')
    expect(structureLabel('elimination')).toBe('Single elimination')
  })

  it.each([
    ['signup', 0, null, 'Sign-up'],
    ['running', 2, 4, 'Round 2 of 4'],
    ['running', 2, null, 'Round 2'],
    ['finished', 4, 4, 'Finished'],
  ] as const)('says where a %s tournament is', (status, round, rounds, text) => {
    expect(statusLabel(status, round, rounds)).toBe(text)
  })

  it.each([
    [match(null, null), 'Ann has a bye'],
    [match(1, null), 'Playing'],
    [match(1, 'a'), 'Ann won'],
    [match(1, 'b'), 'Bob won'],
    [match(1, 'draw'), 'Draw'],
  ])('describes a result', (played, text) => {
    expect(outcomeText(ENTRANTS, played)).toBe(text)
  })

  it('shows fractions as percentages with one decimal', () => {
    expect(percent(1 / 3)).toBe('33.3%')
    expect(percent(1)).toBe('100.0%')
    expect(percent(0)).toBe('0.0%')
  })

  it('knows the player’s matches and unknown entrants', () => {
    expect(isMine(ENTRANTS, match(1, null))).toBe(true)
    expect(isMine([entrant(0, 'Ann'), entrant(1, 'Bob')], match(1, null))).toBe(false)
    expect(entrantName(ENTRANTS, 7)).toBe('Entrant 8')
  })

  it('waits for every result of the current round', () => {
    const view = (matches: MatchView[]): TournamentView =>
      ({ round: 1, rounds_played: [{ round: 1, matches }] }) as unknown as TournamentView
    expect(roundDecided(view([match(1, 'a'), match(null, null)]))).toBe(true)
    expect(roundDecided(view([match(1, null)]))).toBe(false)
  })

  it('links the invite on this origin', () => {
    expect(inviteLink('https://magic.test', 'ABCD2345')).toBe('https://magic.test/tournaments/join/ABCD2345')
  })

  it('offers teams with an open place only in a team format', () => {
    const teams = [entrant(0, 'Red', ['Ann']), entrant(1, 'Blue', ['Bob', 'Cyd'])]
    expect(openTeams(teams, 'commander_2v2').map((t) => t.name)).toEqual(['Red'])
    expect(openTeams(teams, 'commander_1v1')).toEqual([])
  })
})
