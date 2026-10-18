// Goal: every way an account call can fail is put in plain words and sent to the right field, a wrong sign-in never says which half was wrong, and nothing the person typed is echoed back.
import { describe, expect, it } from 'vitest'
import { AccountError, AUTH_ERROR_CODES } from '@/net/accounts'
import type { AccountErrorCode } from '@/net/accounts'
import { accountErrorText, describeAccountError, waitText } from './errors'
import type { AuthMode } from './errors'

const ALL_CODES: readonly AccountErrorCode[] = [...AUTH_ERROR_CODES, 'unknown', 'network', 'bad_response']
const MODES: AuthMode[] = ['login', 'register']
const error = (code: AccountErrorCode, extra: { status?: number; message?: string; wait?: number } = {}) =>
  new AccountError(code, extra.status ?? 400, extra.message ?? 'server words', extra.wait ?? null)

describe('describeAccountError', () => {
  const cases = ALL_CODES.flatMap((code) => MODES.map((mode) => [code, mode] as const))

  it.each(cases)('has a sentence for %s on the %s form', (code, mode) => {
    const shown = describeAccountError(error(code), mode)
    expect(shown.message.length).toBeGreaterThan(20)
    expect(shown.message).toMatch(/[.”"]$/)
    expect(shown.message).not.toContain(code)
    expect(shown.message).not.toMatch(/undefined|null|\[object/)
  })

  it.each([
    ['invalid_username', 'username'],
    ['username_taken', 'username'],
    ['invalid_password', 'password'],
    ['invalid_credentials', 'form'],
    ['too_many_attempts', 'form'],
    ['registration_closed', 'form'],
    ['accounts_unavailable', 'form'],
    ['unavailable', 'form'],
    ['internal', 'form'],
    ['network', 'form'],
    ['forbidden_origin', 'form'],
  ] as const)('puts %s on the %s', (code, field) => {
    expect(describeAccountError(error(code), 'register').field).toBe(field)
  })

  it('says the same thing for a wrong username and a wrong password, and never names either half', () => {
    const shown = describeAccountError(error('invalid_credentials', { status: 401 }), 'login')
    expect(shown.message).toMatch(/username or password is wrong/i)
    expect(shown.message).not.toMatch(/the (username|password) (is|was|does)/i)
    expect(shown.message).not.toMatch(/no such|unknown user|not found|exist/i)
  })

  it('tells a taken username apart from a wrong one and says case does not matter', () => {
    const shown = describeAccountError(error('username_taken', { status: 409 }), 'register')
    expect(shown.message).toMatch(/already taken/i)
    expect(shown.message).toMatch(/upper and lower case/i)
  })

  it('says to wait, for how long when the server said, on both forms', () => {
    for (const mode of MODES) {
      const long = describeAccountError(error('too_many_attempts', { wait: 900 }), mode).message
      expect(long).toMatch(/wait about 15 minutes/i)
      const unknown = describeAccountError(error('too_many_attempts'), mode).message
      expect(unknown).toMatch(/wait a few minutes/i)
    }
  })

  it('names the rules when the server refuses a username or a password', () => {
    expect(describeAccountError(error('invalid_username'), 'register').message).toMatch(/3 to 32 characters/)
    expect(describeAccountError(error('invalid_password'), 'register').message).toMatch(
      /10 to 128 characters/,
    )
  })

  it('says a closed server takes no new accounts and that guests can still play', () => {
    expect(describeAccountError(error('registration_closed'), 'register').message).toMatch(
      /not taking new accounts.*guest/i,
    )
  })

  it('says a server without accounts has none, and that a seat link still works', () => {
    expect(describeAccountError(error('accounts_unavailable'), 'login').message).toMatch(
      /no accounts.*seat link/i,
    )
  })

  it('quotes the server for a code it does not know, and the status for an unreadable answer', () => {
    expect(describeAccountError(error('unknown', { message: 'short and stout' }), 'login').message).toContain(
      '"short and stout"',
    )
    expect(describeAccountError(error('bad_response', { status: 502 }), 'login').message).toContain(
      'status 502',
    )
    expect(describeAccountError(error('bad_response', { status: 0 }), 'login').message).not.toContain(
      'status 0',
    )
  })

  it('does not echo the server’s message for a code it knows', () => {
    const shown = describeAccountError(
      error('invalid_credentials', { message: 'user ada not found' }),
      'login',
    )
    expect(shown.message).not.toContain('ada')
  })

  it.each([new Error('boom'), 'text', null, undefined, 7])(
    'apologises for %j, which is no AccountError',
    (thrown) => {
      expect(describeAccountError(thrown, 'login')).toEqual({
        field: 'form',
        message: 'Something unexpected went wrong. Try again.',
      })
    },
  )

  it('gives accountErrorText as the same sentence without the field', () => {
    const failure = error('network', { status: 0 })
    expect(accountErrorText(failure)).toBe(describeAccountError(failure, 'login').message)
  })
})

describe('waitText', () => {
  it.each([
    [null, 'a few minutes'],
    [0, 'a few minutes'],
    [-3, 'a few minutes'],
    [1, '1 second'],
    [30, '30 seconds'],
    [59, '59 seconds'],
    [60, 'about 1 minute'],
    [61, 'about 2 minutes'],
    [900, 'about 15 minutes'],
    [3599, 'about 1 hour'],
    [3600, 'about 1 hour'],
    [3601, 'about 2 hours'],
    [86_400, 'about 24 hours'],
  ])('writes %j seconds as %j', (seconds, words) => {
    expect(waitText(seconds)).toBe(words)
  })
})
