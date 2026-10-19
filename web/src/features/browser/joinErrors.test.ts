// Goal: every way a join can fail has words for a player, a seat that is gone for good (taken, started, not open) is told apart from a failure that sending again can mend, and the sentence for a 404 says nothing about which of several causes it was, for a row and for a code alike.
import { describe, expect, it } from 'vitest'
import { describeLobbyError, LOBBY_ERROR_CODES, LobbyError } from '@/net/lobbyErrors'
import { codeFailure, joinFailure, NO_GAME_WITH_CODE, NOT_OPEN } from './joinErrors'

const GONE = new Set(['full', 'started', 'not_found'])

describe('every code the server sends', () => {
  it.each(LOBBY_ERROR_CODES)('(%s) has words, and is final only when the seat is gone', (code) => {
    const failure = joinFailure(new LobbyError(code, 400, 'server words', 7))
    expect(failure.text.length).toBeGreaterThan(10)
    expect(failure.final).toBe(GONE.has(code))
  })

  it.each(['network', 'bad_response', 'unknown'] as const)('(%s, raised here) can be sent again', (code) => {
    expect(joinFailure(new LobbyError(code, 0, 'x')).final).toBe(false)
  })
})

describe('the words', () => {
  it.each([
    ['full', 'Someone took that seat'],
    ['started', 'already started'],
  ] as const)('for %s are the shared sentence', (code, part) => {
    const error = new LobbyError(code, 409, 'x')
    expect(joinFailure(error)).toEqual({ text: describeLobbyError(error), final: true })
    expect(joinFailure(error).text).toContain(part)
  })

  it('for a wait carry the seconds the server asked for', () => {
    const failure = joinFailure(new LobbyError('too_many_attempts', 429, 'x', 12))
    expect(failure.text).toContain('Try again in 12 seconds.')
  })

  it('for an error that is not a LobbyError are a general apology, to send again', () => {
    expect(joinFailure(new TypeError('boom'))).toEqual({
      text: 'Something unexpected went wrong. Try again.',
      final: false,
    })
  })

  it('for a 404 are one sentence however the server worded it, and it names no cause', () => {
    const texts = ['unknown game', 'wrong code', 'invite game'].map(
      (said) => joinFailure(new LobbyError('not_found', 404, said)).text,
    )
    expect(new Set(texts)).toEqual(new Set([NOT_OPEN]))
    expect(NOT_OPEN).not.toMatch(/code|private|invite|unknown|wrong|exist/i)
  })
})

describe('a join with a code', () => {
  it('has one sentence for a 404 however the server worded it, and it names no cause', () => {
    const failures = ['unknown game', 'wrong code', 'invite game'].map((said) =>
      codeFailure(new LobbyError('not_found', 404, said)),
    )
    expect(failures).toEqual(Array.from({ length: 3 }, () => ({ text: NO_GAME_WITH_CODE, final: true })))
    expect(NO_GAME_WITH_CODE).toBe('No open game with that code.')
    expect(NO_GAME_WITH_CODE).not.toMatch(/private|invite|unknown|wrong|exist|full|started/i)
  })

  it.each(LOBBY_ERROR_CODES.filter((code) => code !== 'not_found'))(
    '(%s) is worded and final as for a row',
    (code) => {
      const error = new LobbyError(code, 400, 'server words', 7)
      expect(codeFailure(error)).toEqual(joinFailure(error))
    },
  )

  it.each(['network', 'bad_response', 'unknown'] as const)('(%s, raised here) can be sent again', (code) => {
    expect(codeFailure(new LobbyError(code, 0, 'x')).final).toBe(false)
  })

  it('for an error that is not a LobbyError is a general apology, to send again', () => {
    expect(codeFailure(new TypeError('boom'))).toEqual(joinFailure(new TypeError('boom')))
  })
})
