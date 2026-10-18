/**
 * The server's rules for usernames and passwords (docs/server.md, Accounts), repeated here so a form can say what is
 * wrong before it sends anything. The server stays the judge: ts-rs does not export the constants of
 * `mtg_protocol::rest`, so these are copies of them.
 */

/** Fewest characters in a username. */
export const USERNAME_MIN_CHARS = 3
/** Most characters in a username. */
export const USERNAME_MAX_CHARS = 32
/** Fewest characters in a password. */
export const PASSWORD_MIN_CHARS = 10
/** Most characters in a password. */
export const PASSWORD_MAX_CHARS = 128

const USERNAME_SHAPE = /^[A-Za-z0-9_-]*$/

/** The username rule as one sentence for a hint under the field. */
export const USERNAME_RULE = `${USERNAME_MIN_CHARS} to ${USERNAME_MAX_CHARS} characters: letters, digits, _ and -.`
/** The password rule as one sentence for a hint under the field. */
export const PASSWORD_RULE = `${PASSWORD_MIN_CHARS} to ${PASSWORD_MAX_CHARS} characters, spaces and symbols included.`

/** What is wrong with `username`, as a sentence for the person typing, or null. Nothing is trimmed, as on the server. */
export function usernameProblem(username: string): string | null {
  if (username === '') return 'Enter a username.'
  if (!USERNAME_SHAPE.test(username)) return 'Use only letters, digits, _ and - in a username (no spaces).'
  if (username.length < USERNAME_MIN_CHARS) {
    return `A username needs at least ${USERNAME_MIN_CHARS} characters (this one has ${username.length}).`
  }
  if (username.length > USERNAME_MAX_CHARS) {
    return `A username has at most ${USERNAME_MAX_CHARS} characters (this one has ${username.length}).`
  }
  return null
}

/** What is wrong with `password`, or null. Counted in characters, as the server does, and never trimmed. */
export function passwordProblem(password: string): string | null {
  if (password === '') return 'Enter a password.'
  const length = [...password].length
  if (length < PASSWORD_MIN_CHARS) {
    return `A password needs at least ${PASSWORD_MIN_CHARS} characters (this one has ${length}).`
  }
  if (length > PASSWORD_MAX_CHARS) {
    return `A password has at most ${PASSWORD_MAX_CHARS} characters (this one has ${length}).`
  }
  return null
}
