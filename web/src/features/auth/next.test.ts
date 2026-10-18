// Goal: the sign-in pages follow a `next` address only when it is a plain path on this site, so no crafted link can send a person to another site after they sign in, and a seat token never travels in it.
import { describe, expect, it } from 'vitest'
import { MAX_NEXT_CHARS, parseAuthSearch, sanitizeNext } from './next'

describe('sanitizeNext accepts paths on this site', () => {
  it.each([
    ['/', '/'],
    ['/games', '/games'],
    ['/games?sort=new', '/games?sort=new'],
    ['/game/8f2c1a52-31de-4c77-9a35-0d1d3f6a7b10', '/game/8f2c1a52-31de-4c77-9a35-0d1d3f6a7b10'],
    ['/a/b/c?x=1&y=2', '/a/b/c?x=1&y=2'],
    ['/dev/table?seats=4', '/dev/table?seats=4'],
    ['/a/./b', '/a/b'],
    ['/a/b/../c', '/a/c'],
    ['/games/', '/games/'],
    ['/path with space', '/path%20with%20space'],
    ['/café', '/caf%C3%A9'],
    ['/%2F/evil.example', '/%2F/evil.example'],
    ['/%5Cevil.example', '/%5Cevil.example'],
    ['/https://evil.example', '/https://evil.example'],
    ['/login-help', '/login-help'],
    ['/loginx', '/loginx'],
  ])('keeps %j as %j', (value, expected) => {
    expect(sanitizeNext(value)).toBe(expected)
  })

  it('drops the fragment, so a seat token cannot ride in a query string', () => {
    expect(sanitizeNext('/game/abc#seat=1&token=deadbeef')).toBe('/game/abc')
    expect(sanitizeNext('/games?x=1#top')).toBe('/games?x=1')
  })
})

describe('sanitizeNext refuses everything else', () => {
  it.each([
    ['nothing', ''],
    ['a bare word', 'games'],
    ['a relative path', './games'],
    ['a parent path', '../games'],
    ['a query only', '?next=/games'],
    ['a fragment only', '#games'],
    ['a protocol-relative address', '//evil.example'],
    ['a protocol-relative address with a path', '//evil.example/games'],
    ['three slashes', '///evil.example'],
    ['a protocol-relative address with userinfo', '//user:pass@evil.example'],
    ['an absolute https address', 'https://evil.example'],
    ['an absolute http address', 'http://evil.example/games'],
    ['the page’s own origin written out', 'https://app.invalid/games'],
    ['a javascript address', 'javascript:alert(1)'],
    ['a data address', 'data:text/html,<script>alert(1)</script>'],
    ['a mailto address', 'mailto:a@b.example'],
    ['a slash then a backslash', '/\\evil.example'],
    ['two backslashes', '\\\\evil.example'],
    ['a backslash later in the path', '/games\\..\\evil'],
    ['a slash, a tab, a slash', '/\t/evil.example'],
    ['a slash, a newline, a slash', '/\n/evil.example'],
    ['a slash, a carriage return, a slash', '/\r/evil.example'],
    ['a NUL byte', '/games\u0000'],
    ['a DEL character', '/games\u007f'],
    ['a leading space', ' /games'],
    ['a leading tab', '\t/games'],
    ['a leading newline', '\n/games'],
    ['dot segments that leave two slashes', '/.//evil.example'],
    ['dot segments that climb out of the path', '/..//evil.example'],
    ['an encoded protocol-relative address that was decoded twice', '%2F%2Fevil.example'],
    ['a path longer than the limit', `/${'a'.repeat(MAX_NEXT_CHARS)}`],
  ])('refuses %s', (_what, value) => {
    expect(sanitizeNext(value)).toBeNull()
  })

  it.each([
    ['undefined', undefined],
    ['null', null],
    ['a number (the router parses 5 as a number)', 5],
    ['true', true],
    ['an array', ['/games']],
    ['an object', { to: '/games' }],
  ])('refuses %s', (_what, value) => {
    expect(sanitizeNext(value)).toBeNull()
  })

  it.each(['/login', '/register', '/login/', '/LOGIN', '/login?next=/games', '/register#x', '/Register/'])(
    'refuses %j, which would only lead back to the sign-in form',
    (value) => {
      expect(sanitizeNext(value)).toBeNull()
    },
  )

  it('accepts a path exactly at the limit', () => {
    const path = `/${'a'.repeat(MAX_NEXT_CHARS - 1)}`
    expect(path).toHaveLength(MAX_NEXT_CHARS)
    expect(sanitizeNext(path)).toBe(path)
  })
})

describe('parseAuthSearch', () => {
  it('keeps a good next and drops a bad one, without throwing', () => {
    expect(parseAuthSearch({ next: '/games' })).toEqual({ next: '/games' })
    expect(parseAuthSearch({ next: '//evil.example' })).toEqual({})
    expect(parseAuthSearch({ next: 7 })).toEqual({})
    expect(parseAuthSearch({})).toEqual({})
  })

  it('ignores every other parameter', () => {
    expect(parseAuthSearch({ next: '/games', redirect: 'https://evil.example', x: 1 })).toEqual({
      next: '/games',
    })
  })
})
