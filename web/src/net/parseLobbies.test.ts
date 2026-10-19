// Goal: every answer of the game browser endpoints is read down to its leaves: a good body gives exactly its documented fields, and a missing, mistyped or impossible field gives null instead of a half-typed value.
import { describe, expect, it } from 'vitest'
import { isVisibility, readJoined, readLobbyGame, readLobbyList, readVisibility } from './parseLobbies'

const row = {
  game: '6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55',
  host: 'Ann',
  format: 'normal',
  filled: 1,
  total: 4,
  created_at: '2026-10-19T12:30:00Z',
  age_secs: 125,
}

describe('readLobbyGame', () => {
  it('reads a row and keeps only its documented fields', () => {
    expect(readLobbyGame({ ...row, token: 'secret', code: 'ABCD2345', later: true })).toEqual(row)
  })

  it.each<[string, number, number]>([
    ['an empty lobby of the host alone', 1, 2],
    ['a full lobby', 4, 4],
    ['twelve seats', 7, 12],
    ['no seat counted', 0, 3],
  ])('reads %s', (_what, filled, total) => {
    expect(readLobbyGame({ ...row, filled, total })).toEqual({ ...row, filled, total })
  })

  it.each(Object.keys(row))('is null without %s', (key) => {
    const { [key]: _gone, ...rest } = row as Record<string, unknown>
    expect(readLobbyGame(rest)).toBeNull()
  })

  it.each<[string, Record<string, unknown>]>([
    ['game is a number', { game: 7 }],
    ['host is null', { host: null }],
    ['format is a list', { format: ['normal'] }],
    ['filled is text', { filled: '1' }],
    ['filled is negative', { filled: -1 }],
    ['filled is a fraction', { filled: 1.5 }],
    ['total is infinite', { total: Infinity }],
    ['total is not a number', { total: Number.NaN }],
    ['age is negative', { age_secs: -3 }],
    ['age is a fraction', { age_secs: 1.25 }],
    ['age is text', { age_secs: '125' }],
    ['more seats are filled than there are', { filled: 5, total: 4 }],
    ['the stamp has no time', { created_at: '2026-10-19' }],
    ['the stamp has an offset', { created_at: '2026-10-19T12:30:00+02:00' }],
    ['the stamp has fractions of a second', { created_at: '2026-10-19T12:30:00.000Z' }],
    ['the stamp is lower case', { created_at: '2026-10-19t12:30:00z' }],
    ['the stamp is a number', { created_at: 1_760_877_000 }],
  ])('is null when %s', (_what, change) => {
    expect(readLobbyGame({ ...row, ...change })).toBeNull()
  })

  it.each<[string, unknown]>([
    ['null', null],
    ['a list', [row]],
    ['text', 'row'],
    ['a number', 7],
  ])('is null for %s', (_what, value) => {
    expect(readLobbyGame(value)).toBeNull()
  })
})

describe('readLobbyList', () => {
  it('reads the rows in the order they came', () => {
    const second = { ...row, game: 'g-2', host: 'Bo', age_secs: 10 }
    expect(readLobbyList({ games: [row, second] })).toEqual([row, second])
  })

  it('reads an empty list', () => {
    expect(readLobbyList({ games: [] })).toEqual([])
  })

  it('reads fifty rows', () => {
    const many = Array.from({ length: 50 }, (_, n) => ({ ...row, game: `g-${n}` }))
    expect(readLobbyList({ games: many })).toHaveLength(50)
  })

  it('is null when a single row is unreadable, rather than a shorter list', () => {
    expect(readLobbyList({ games: [row, { ...row, filled: 9 }, row] })).toBeNull()
    expect(readLobbyList({ games: [row, null] })).toBeNull()
  })

  it.each<[string, unknown]>([
    ['a bare list', [row]],
    ['no games', {}],
    ['games as an object', { games: { 0: row } }],
    ['games as null', { games: null }],
    ['null', null],
    ['text', 'games'],
  ])('is null for %s', (_what, body) => {
    expect(readLobbyList(body)).toBeNull()
  })
})

describe('readJoined', () => {
  it('reads the seat and its token', () => {
    expect(readJoined({ seat: 2, token: 'abc' })).toEqual({ seat: 2, token: 'abc' })
  })

  it('reads a null token, which is how an account that already held a seat is told', () => {
    expect(readJoined({ seat: 0, token: null })).toEqual({ seat: 0, token: null })
  })

  it('drops fields it does not know', () => {
    expect(readJoined({ seat: 1, token: 't', code: 'ABCD2345' })).toEqual({ seat: 1, token: 't' })
  })

  it.each<[string, unknown]>([
    ['no token key at all', { seat: 1 }],
    ['a numeric token', { seat: 1, token: 5 }],
    ['a list as the token', { seat: 1, token: ['t'] }],
    ['no seat', { token: 't' }],
    ['a negative seat', { seat: -1, token: 't' }],
    ['a fractional seat', { seat: 0.5, token: 't' }],
    ['a text seat', { seat: '1', token: 't' }],
    ['null', null],
    ['a list', [1, 't']],
  ])('is null for %s', (_what, body) => {
    expect(readJoined(body)).toBeNull()
  })
})

describe('visibility', () => {
  it.each(['public', 'invite'])('knows %s', (word) => {
    expect(isVisibility(word)).toBe(true)
    expect(readVisibility({ visibility: word })).toBe(word)
  })

  it.each<[string, unknown]>([
    ['another case', 'Public'],
    ['another word', 'private'],
    ['an empty word', ''],
    ['a number', 1],
    ['null', null],
    ['undefined', undefined],
    ['an inherited name', 'constructor'],
    ['a prototype key', '__proto__'],
  ])('does not know %s', (_what, value) => {
    expect(isVisibility(value)).toBe(false)
    expect(readVisibility({ visibility: value })).toBeNull()
  })

  it('needs a body with the field', () => {
    expect(readVisibility({})).toBeNull()
    expect(readVisibility(null)).toBeNull()
    expect(readVisibility('public')).toBeNull()
  })
})
