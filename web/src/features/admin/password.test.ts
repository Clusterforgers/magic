// Goal: a generated password has exactly 16 characters, all from the alphabet without look-alikes, comes from the random source it is given with every character equally likely, passes the server's password rule and differs from one call to the next.
import { describe, expect, it, vi } from 'vitest'
import { passwordProblem } from '@/features/auth/rules'
import { GENERATED_PASSWORD_LENGTH, generatePassword, PASSWORD_ALPHABET } from './password'
import type { RandomFill } from './password'

/** A source that hands out `bytes` in order, repeating them when they run out. */
function sequence(bytes: number[]): RandomFill {
  let at = 0
  return (buffer) => {
    for (let i = 0; i < buffer.length; i += 1) {
      buffer[i] = bytes[at % bytes.length] ?? 0
      at += 1
    }
    return buffer
  }
}

describe('generatePassword', () => {
  it('fills the password with 16 characters from the browser’s random source', () => {
    const password = generatePassword()
    expect(password).toHaveLength(GENERATED_PASSWORD_LENGTH)
    expect(GENERATED_PASSWORD_LENGTH).toBe(16)
    for (const character of password) expect(PASSWORD_ALPHABET).toContain(character)
    expect(passwordProblem(password)).toBeNull()
  })

  it('draws every character from crypto.getRandomValues, never Math.random', () => {
    const random = vi.spyOn(Math, 'random').mockImplementation(() => {
      throw new Error('Math.random must not make a password')
    })
    const secure = vi.spyOn(globalThis.crypto, 'getRandomValues')
    try {
      expect(generatePassword()).toHaveLength(16)
      expect(secure).toHaveBeenCalled()
    } finally {
      random.mockRestore()
      secure.mockRestore()
    }
  })

  it('leaves out the characters that are mistaken for one another', () => {
    for (const confusing of '0O1Il|') expect(PASSWORD_ALPHABET).not.toContain(confusing)
    expect(new Set(PASSWORD_ALPHABET).size).toBe(PASSWORD_ALPHABET.length)
    expect(PASSWORD_ALPHABET).toMatch(/^[A-Za-z2-9]+$/)
  })

  it('maps bytes to characters in order, so each character is as likely as any other', () => {
    const size = PASSWORD_ALPHABET.length
    const bytes = Array.from({ length: GENERATED_PASSWORD_LENGTH }, (_, n) => n)
    const password = generatePassword(sequence(bytes))
    expect([...password].map((character) => PASSWORD_ALPHABET.indexOf(character))).toEqual(
      bytes.map((byte) => byte % size),
    )
  })

  it('throws away the bytes that would make some characters likelier than others', () => {
    const size = PASSWORD_ALPHABET.length
    const limit = 256 - (256 % size)
    // Every byte from `limit` up is refused; the accepted ones follow in order.
    const refused = Array.from({ length: 256 - limit }, (_, n) => limit + n)
    const password = generatePassword(sequence([...refused, 0, 1, 2, 3, ...refused, 4, 5]))
    expect(password.slice(0, 6)).toBe([0, 1, 2, 3, 4, 5].map((n) => PASSWORD_ALPHABET[n]).join(''))
    expect(password).toHaveLength(16)
  })

  it('differs from one call to the next', () => {
    const passwords = new Set(Array.from({ length: 50 }, () => generatePassword()))
    expect(passwords.size).toBe(50)
  })

  it('uses every character of the alphabet over many passwords', () => {
    const seen = new Set<string>()
    for (let n = 0; n < 400; n += 1) for (const character of generatePassword()) seen.add(character)
    expect(seen.size).toBe(PASSWORD_ALPHABET.length)
  })
})
