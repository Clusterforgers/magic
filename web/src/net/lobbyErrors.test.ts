// Goal: a failed game browser call keeps the server's code, status, message and wait; a code this page has not heard of is `unknown`; and every code, local ones included, has its own sentence in plain words that names the wait when there is one.
import { describe, expect, it } from 'vitest'
import {
  LOBBY_ERROR_CODES,
  LobbyError,
  describeLobbyError,
  isLobbyErrorCode,
  lobbyFailure,
} from './lobbyErrors'
import type { LobbyFailureCode } from './lobbyErrors'

/** The codes of `crates/mtg-protocol/src/rest/lobbies.rs`, in the order Rust declares them. */
const SERVER_CODES = [
  'invalid_request',
  'invalid_name',
  'unauthenticated',
  'forbidden_origin',
  'forbidden',
  'not_found',
  'full',
  'started',
  'not_claimed',
  'payload_too_large',
  'unsupported_media_type',
  'too_many_attempts',
  'rate_limited',
  'accounts_unavailable',
  'lobbies_unavailable',
  'unavailable',
  'internal',
] as const

const LOCAL_CODES = ['unknown', 'network', 'bad_response'] as const

function answer(status: number, headers: Record<string, string> = {}): Response {
  return new Response(null, { status, headers })
}

describe('the codes', () => {
  it('are the ones the server sends, once each', () => {
    expect(LOBBY_ERROR_CODES).toEqual(SERVER_CODES)
    expect(new Set(LOBBY_ERROR_CODES).size).toBe(LOBBY_ERROR_CODES.length)
  })

  it.each([...SERVER_CODES])('knows %s', (code) => {
    expect(isLobbyErrorCode(code)).toBe(true)
  })

  it.each(['', 'Full', 'teapot', 'constructor', '__proto__', 'toString', 'unknown', 'network'])(
    'does not know "%s" as a server code',
    (code) => {
      expect(isLobbyErrorCode(code)).toBe(false)
    },
  )
})

describe('lobbyFailure', () => {
  it.each([...SERVER_CODES])('keeps the code %s, the status and the server message', (code) => {
    const error = lobbyFailure(answer(409), { error: code, message: 'in the server words' })
    expect(error).toBeInstanceOf(LobbyError)
    expect(error).toBeInstanceOf(Error)
    expect(error.name).toBe('LobbyError')
    expect({ code: error.code, status: error.status, message: error.message }).toEqual({
      code,
      status: 409,
      message: 'in the server words',
    })
    expect(error.retryAfter).toBeNull()
  })

  it('calls a code it has not heard of unknown and keeps what the server said', () => {
    const error = lobbyFailure(answer(418), { error: 'teapot', message: 'short and stout' })
    expect([error.code, error.status, error.message]).toEqual(['unknown', 418, 'short and stout'])
  })

  it.each<[string, string, number | null]>([
    ['whole seconds', '30', 30],
    ['padded seconds', ' 7 ', 7],
    ['one second', '1', 1],
    ['nine digits', '999999999', 999_999_999],
    ['an HTTP date', 'Wed, 21 Oct 2026 07:28:00 GMT', null],
    ['a fraction', '1.5', null],
    ['a negative number', '-5', null],
    ['ten digits', '1000000000', null],
    ['nothing', '', null],
  ])('reads Retry-After as %s', (_what, header, seconds) => {
    const error = lobbyFailure(answer(429, { 'retry-after': header }), {
      error: 'too_many_attempts',
      message: 'wait',
    })
    expect(error.retryAfter).toBe(seconds)
  })

  it('has no wait without the header', () => {
    expect(lobbyFailure(answer(429), { error: 'rate_limited', message: 'wait' }).retryAfter).toBeNull()
  })

  it.each<[string, unknown]>([
    ['no body', undefined],
    ['null', null],
    ['a list', []],
    ['text', 'service unavailable'],
    ['an error without a message', { error: 'full' }],
    ['a code that is not text', { error: 7, message: 'seven' }],
    ['a message that is not text', { error: 'full', message: 7 }],
  ])('is a bad_response with the status kept for %s', (_what, body) => {
    const error = lobbyFailure(answer(502), body)
    expect([error.code, error.status]).toEqual(['bad_response', 502])
    expect(error.message).toContain('502')
  })
})

describe('describeLobbyError', () => {
  const all: LobbyFailureCode[] = [...SERVER_CODES, ...LOCAL_CODES]
  const words = (code: LobbyFailureCode, retryAfter: number | null = null): string =>
    describeLobbyError(new LobbyError(code, 400, 'server message', retryAfter))

  it.each(all)('has a sentence for %s', (code) => {
    const text = words(code)
    expect(text.length).toBeGreaterThan(20)
    expect(text).toMatch(/[.!?]$/)
    // A player reads words, not the machine's code.
    expect(text).not.toMatch(/[a-z]+_[a-z]+/)
  })

  it('gives every code a sentence of its own', () => {
    const sentences = all.map((code) => words(code))
    expect(new Set(sentences).size).toBe(sentences.length)
  })

  it.each<[LobbyFailureCode, number | null, string]>([
    ['too_many_attempts', 12, 'in 12 seconds'],
    ['too_many_attempts', 1, 'in 1 second.'],
    ['too_many_attempts', null, 'in a minute'],
    ['rate_limited', 30, 'in 30 seconds'],
    ['rate_limited', 1, 'in 1 second.'],
    ['rate_limited', null, 'in a minute'],
  ])('says how long to wait for %s (%s)', (code, wait, expected) => {
    expect(words(code, wait)).toContain(expected)
  })

  it('tells the reasons a join was refused apart', () => {
    expect(words('full')).toMatch(/full|took/)
    expect(words('started')).toMatch(/started/)
    expect(words('not_found')).toMatch(/code|started|closed/)
    expect(words('invalid_name')).toMatch(/1 to 32/)
    expect(words('forbidden')).toMatch(/host/)
  })

  it('quotes the server for a code it does not know, and the status for an unreadable answer', () => {
    const unknown = describeLobbyError(new LobbyError('unknown', 418, 'short and stout'))
    expect(unknown).toContain('"short and stout"')
    const unreadable = describeLobbyError(new LobbyError('bad_response', 502, 'x'))
    expect(unreadable).toContain('502')
    expect(describeLobbyError(new LobbyError('bad_response', 0, 'x'))).not.toContain('status')
  })

  it.each<[string, unknown]>([
    ['a plain error', new Error('boom')],
    ['text', 'boom'],
    ['null', null],
    ['undefined', undefined],
  ])('apologises for %s', (_what, error) => {
    expect(describeLobbyError(error)).toBe('Something unexpected went wrong. Try again.')
  })
})
