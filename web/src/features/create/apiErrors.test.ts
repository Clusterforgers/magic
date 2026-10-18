// Goal: every code a REST call can fail with maps to its own plain-words message that names no internal code, and anything else gets a general one.
import { describe, expect, it } from 'vitest'
import { ApiError, SERVER_ERROR_CODES } from '@/net/api'
import type { ApiErrorCode } from '@/net/api'
import { describeApiError } from './apiErrors'

const ALL_CODES: ApiErrorCode[] = [
  'invalid_request',
  'unsupported_format',
  'unknown_deck',
  'forbidden_origin',
  'not_found',
  'method_not_allowed',
  'payload_too_large',
  'unsupported_media_type',
  'internal',
  'server_full',
  'unavailable',
  'unknown',
  'network',
  'bad_response',
]

const fail = (code: ApiErrorCode, status = 400, message = 'server words'): ApiError =>
  new ApiError(code, status, message)

describe('describeApiError', () => {
  it('lists every code the server can send, so a new one fails here until it has a message', () => {
    for (const code of SERVER_ERROR_CODES) expect(ALL_CODES).toContain(code)
    expect(ALL_CODES).toHaveLength(SERVER_ERROR_CODES.length + 3)
  })

  it.each(ALL_CODES)('says something useful for %s, without repeating the code', (code) => {
    const text = describeApiError(fail(code))
    expect(text.length).toBeGreaterThan(30)
    expect(text).toMatch(/^[A-Z]/)
    expect(text).toMatch(/[.]$/)
    expect(text).not.toContain(code)
    expect(text).not.toMatch(/_|undefined|\[object|HTTP|JSON|payload|origin\b/i)
  })

  it('gives each kind of failure its own message, except the two that mean the page is out of date', () => {
    const texts = ALL_CODES.map((code) => describeApiError(fail(code, 502)))
    const duplicates = texts.filter((text, i) => texts.indexOf(text) !== i)
    expect(duplicates).toHaveLength(1)
    expect(describeApiError(fail('method_not_allowed'))).toBe(
      describeApiError(fail('unsupported_media_type')),
    )
  })

  it('tells a player what to do next', () => {
    expect(describeApiError(fail('network'))).toMatch(/check your connection/i)
    expect(describeApiError(fail('server_full'))).toMatch(/try again/i)
    expect(describeApiError(fail('unavailable'))).toMatch(/try again/i)
    expect(describeApiError(fail('unknown_deck'))).toMatch(/reload/i)
    expect(describeApiError(fail('invalid_request'))).toMatch(/2 to 12 seats/)
  })

  it('passes on what the server said when it sent a code this page does not know', () => {
    expect(describeApiError(fail('unknown', 418, 'I am a teapot'))).toContain('I am a teapot')
  })

  it('mentions the status when a body could not be read, and omits it when there was no answer at all', () => {
    expect(describeApiError(fail('bad_response', 502))).toContain('(status 502)')
    expect(describeApiError(fail('bad_response', 0))).not.toContain('status')
  })

  it.each([
    ['a plain Error', new Error('boom')],
    ['a string', 'boom'],
    ['undefined', undefined],
    ['null', null],
  ])('gives a general message for %s', (_what, value) => {
    expect(describeApiError(value)).toBe('Something unexpected went wrong. Try again.')
  })
})
