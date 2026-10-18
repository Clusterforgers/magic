// Goal: the username and password checks accept exactly what the server accepts (3 to 32 ASCII letters, digits, _ and -; 10 to 128 characters, nothing trimmed) and say plainly what is wrong with the rest.
import { describe, expect, it } from 'vitest'
import {
  PASSWORD_MAX_CHARS,
  PASSWORD_MIN_CHARS,
  passwordProblem,
  USERNAME_MAX_CHARS,
  USERNAME_MIN_CHARS,
  usernameProblem,
} from './rules'

describe('usernameProblem', () => {
  it.each([
    ['the shortest', 'a'.repeat(USERNAME_MIN_CHARS)],
    ['the longest', 'a'.repeat(USERNAME_MAX_CHARS)],
    ['letters of both cases, digits, underscore and hyphen', 'Ada_Lovelace-1815'],
    ['only digits', '123'],
    ['only symbols from the set', '_-_'],
  ])('accepts %s', (_what, name) => {
    expect(usernameProblem(name)).toBeNull()
  })

  it.each([
    ['nothing', '', /enter a username/i],
    ['two characters', 'ab', /at least 3 characters \(this one has 2\)/],
    ['33 characters', 'a'.repeat(USERNAME_MAX_CHARS + 1), /at most 32 characters \(this one has 33\)/],
    ['a space inside', 'ada l', /only letters, digits, _ and -/],
    ['a leading space (nothing is trimmed)', ' ada', /only letters, digits, _ and -/],
    ['a trailing space (nothing is trimmed)', 'ada ', /only letters, digits, _ and -/],
    ['an at sign', 'ada@example', /only letters, digits, _ and -/],
    ['a dot', 'ada.l', /only letters, digits, _ and -/],
    ['an accented letter', 'adá_l', /only letters, digits, _ and -/],
    ['a look-alike Cyrillic letter', 'аda_l', /only letters, digits, _ and -/],
    ['an emoji', 'ada🙂', /only letters, digits, _ and -/],
    ['a newline', 'ada\nl', /only letters, digits, _ and -/],
    ['a zero-width space', 'ada​l', /only letters, digits, _ and -/],
    ['bad characters and too short', 'a b', /only letters, digits, _ and -/],
  ])('refuses %s', (_what, name, words) => {
    expect(usernameProblem(name)).toMatch(words)
  })
})

describe('passwordProblem', () => {
  it.each([
    ['the shortest', 'x'.repeat(PASSWORD_MIN_CHARS)],
    ['the longest', 'x'.repeat(PASSWORD_MAX_CHARS)],
    ['spaces at the ends (nothing is trimmed)', '  ten spaces  '],
    ['ten spaces', ' '.repeat(PASSWORD_MIN_CHARS)],
    ['ten characters that are many UTF-16 units each', '🙂'.repeat(PASSWORD_MIN_CHARS)],
    ['128 emoji, which are 256 UTF-16 units', '🙂'.repeat(PASSWORD_MAX_CHARS)],
    ['accents and symbols', 'pässwörd!€#'],
  ])('accepts %s', (_what, password) => {
    expect(passwordProblem(password)).toBeNull()
  })

  it.each([
    ['nothing', '', /enter a password/i],
    ['nine characters', 'x'.repeat(PASSWORD_MIN_CHARS - 1), /at least 10 characters \(this one has 9\)/],
    ['nine emoji, though they are eighteen UTF-16 units', '🙂'.repeat(9), /this one has 9\)/],
    ['129 characters', 'x'.repeat(PASSWORD_MAX_CHARS + 1), /at most 128 characters \(this one has 129\)/],
  ])('refuses %s', (_what, password, words) => {
    expect(passwordProblem(password)).toMatch(words)
  })

  it('never repeats the password in what it says', () => {
    const secret = 'hunter2 hunter2 hunter2 hunter2 '.repeat(8)
    expect(passwordProblem(secret)).not.toContain('hunter2')
    expect(passwordProblem('short-pw')).not.toContain('short-pw')
  })
})
