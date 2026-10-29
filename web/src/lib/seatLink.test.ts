// Goal: a seat link carries its token only in the fragment, reads back to the same seat and token, and anything malformed is refused instead of guessed at.
import { describe, expect, it } from 'vitest'
import { fragmentOf, parseSeatFragment, seatLinkPath, seatLinkUrl } from './seatLink'

// Every seat a table can have is a number; these are the ones the cases try.
const SEATS_TRIED = [0, 1, 2, 7, 11, 12, 13, 23, 63, 999]

const GAME = '8f2c1a52-31de-4c77-9a35-0d1d3f6a7b10'
const TOKEN = 'a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90'

describe('seatLinkPath', () => {
  it.each(SEATS_TRIED)('has the documented form for seat %i', (seat) => {
    expect(seatLinkPath(GAME, seat, TOKEN)).toBe(`/game/${GAME}#seat=${seat}&token=${TOKEN}`)
  })

  it('puts the token after the # and nowhere before it', () => {
    const url = new URL(seatLinkUrl('https://play.example', GAME, 1, TOKEN))
    expect(url.origin).toBe('https://play.example')
    expect(url.pathname).toBe(`/game/${GAME}`)
    expect(url.search).toBe('')
    expect(url.hash).toBe(`#seat=1&token=${TOKEN}`)
    expect(`${url.origin}${url.pathname}${url.search}`).not.toContain(TOKEN)
  })

  it.each([
    ['a plain hex token', 'abc123', 'abc123'],
    ['base64 punctuation', 'a+b/c=', 'a%2Bb%2Fc%3D'],
    ['a separator that would end the value', 'x&seat=9', 'x%26seat%3D9'],
    ['a hash sign', 'x#y', 'x%23y'],
  ])('escapes %s in the token so it cannot leak into another field', (_name, token, escaped) => {
    const path = seatLinkPath(GAME, 0, token)
    expect(path).toBe(`/game/${GAME}#seat=0&token=${escaped}`)
    expect(parseSeatFragment(fragmentOf(path))).toEqual({ seat: 0, token })
  })

  it('escapes a game id that is not a plain id', () => {
    expect(seatLinkPath('a/b?c', 0, 't')).toBe('/game/a%2Fb%3Fc#seat=0&token=t')
  })
})

describe('fragmentOf', () => {
  it.each([
    ['https://x.example/game/g#seat=1&token=t', 'seat=1&token=t'],
    ['/game/g#seat=1&token=t', 'seat=1&token=t'],
    ['/game/g', ''],
    ['/game/g#', ''],
    ['/game/g#a#b', 'a#b'],
  ])('%s gives %j', (href, fragment) => {
    expect(fragmentOf(href)).toBe(fragment)
  })
})

describe('parseSeatFragment', () => {
  it.each(SEATS_TRIED)('reads back seat %i of its own link', (seat) => {
    const path = seatLinkPath(GAME, seat, TOKEN)
    expect(parseSeatFragment(fragmentOf(path))).toEqual({ seat, token: TOKEN })
  })

  it('accepts the fragment with its leading # and in either order', () => {
    expect(parseSeatFragment('#seat=2&token=abc')).toEqual({ seat: 2, token: 'abc' })
    expect(parseSeatFragment('token=abc&seat=2')).toEqual({ seat: 2, token: 'abc' })
  })

  it('ignores fields it does not know', () => {
    expect(parseSeatFragment('seat=2&token=abc&x=1')).toEqual({ seat: 2, token: 'abc' })
  })

  it.each([
    ['nothing', ''],
    ['only a hash', '#'],
    ['no token', 'seat=1'],
    ['no seat', 'token=abc'],
    ['an empty token', 'seat=1&token='],
    ['a token with a space', 'seat=1&token=a%20b'],
    ['a token with a control character', 'seat=1&token=a%00b'],
    ['a token that is not ASCII', 'seat=1&token=caf%C3%A9'],
    ['a token of 513 characters', `seat=1&token=${'a'.repeat(513)}`],
    ['a seat that is not a number', 'seat=x&token=abc'],
    ['a negative seat', 'seat=-1&token=abc'],
    ['a fractional seat', 'seat=1.5&token=abc'],
    ['a seat with a leading zero', 'seat=01&token=abc'],
    ['a seat with an exponent', 'seat=1e1&token=abc'],
    ['an empty seat', 'seat=&token=abc'],
    ['a huge seat', 'seat=99999999999999999999&token=abc'],
  ])('refuses %s', (_name, fragment) => {
    expect(parseSeatFragment(fragment)).toBeNull()
  })

  it('takes the first of a repeated field, so a pasted extra token cannot swap the seat', () => {
    expect(parseSeatFragment('seat=1&token=first&token=second')).toEqual({ seat: 1, token: 'first' })
  })
})
