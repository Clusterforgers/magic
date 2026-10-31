/**
 * Passwords the admin hands to a friend: random, 16 characters, from letters and digits that cannot be mistaken for
 * one another (no 0, O, 1, I or l), so the password survives being read aloud or copied from a screen. The random bytes
 * come from `crypto.getRandomValues`; nothing here keeps a password.
 */

/** Characters in a generated password. */
export const GENERATED_PASSWORD_LENGTH = 16

/** The characters a generated password is drawn from. */
export const PASSWORD_ALPHABET = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789'

/** Fills a buffer with random bytes; the browser's `crypto.getRandomValues` by default. */
export type RandomFill = (bytes: Uint8Array<ArrayBuffer>) => Uint8Array

const browserFill: RandomFill = (bytes) => crypto.getRandomValues(bytes)

/**
 * A new random password. A byte is used only when it falls below the largest multiple of the alphabet's size, so every
 * character is equally likely; the others are drawn again.
 */
export function generatePassword(fill: RandomFill = browserFill): string {
  const size = PASSWORD_ALPHABET.length
  const limit = 256 - (256 % size)
  let password = ''
  while (password.length < GENERATED_PASSWORD_LENGTH) {
    const bytes = new Uint8Array(GENERATED_PASSWORD_LENGTH * 2)
    fill(bytes)
    for (const byte of bytes) {
      if (byte < limit && password.length < GENERATED_PASSWORD_LENGTH)
        password += PASSWORD_ALPHABET[byte % size]
    }
  }
  return password
}
