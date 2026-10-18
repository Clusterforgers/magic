/** Plain-words messages for every way an account call can fail, and which field of the form each belongs to. */
import { AccountError } from '@/net/accounts'
import type { AccountErrorCode } from '@/net/accounts'
import { PASSWORD_RULE, USERNAME_RULE } from './rules'

/** Which form this is: the same fields, and different things that can go wrong. */
export type AuthMode = 'login' | 'register'

/** An error ready to show: the field it belongs to, or the whole form, and the sentence. */
export interface ShownError {
  field: 'username' | 'password' | 'form'
  message: string
}

const OUT_OF_DATE =
  'This page and the server do not agree on how to talk to each other. Reload the page; if that does not help, the app needs an update.'
const TRY_LATER = 'The server ran into a problem. Try again in a moment.'

/** "30 seconds", "about 15 minutes", "about 2 hours"; "a few minutes" when the server did not say. */
export function waitText(seconds: number | null): string {
  if (seconds === null || seconds <= 0) return 'a few minutes'
  if (seconds < 60) return seconds === 1 ? '1 second' : `${seconds} seconds`
  const minutes = Math.ceil(seconds / 60)
  if (minutes < 60) return minutes === 1 ? 'about 1 minute' : `about ${minutes} minutes`
  const hours = Math.ceil(minutes / 60)
  return hours === 1 ? 'about 1 hour' : `about ${hours} hours`
}

const form = (message: string): ShownError => ({ field: 'form', message })

// A `Record` over every code, so adding a code to the client stops compiling here until it has a message. Nothing
// here may say which half of a wrong sign-in was wrong, or echo what the person typed.
const MESSAGES: Record<AccountErrorCode, (error: AccountError, mode: AuthMode) => ShownError> = {
  invalid_request: () => form(`The server could not read that form. ${OUT_OF_DATE}`),
  invalid_username: () => ({
    field: 'username',
    message: `The server does not accept that username. Use ${USERNAME_RULE}`,
  }),
  invalid_password: () => ({
    field: 'password',
    message: `The server does not accept that password. Use ${PASSWORD_RULE}`,
  }),
  username_taken: () => ({
    field: 'username',
    message:
      'That username is already taken (upper and lower case count as the same). Choose another one, or sign in if it is yours.',
  }),
  registration_closed: () =>
    form(
      'This server is not taking new accounts right now. Ask whoever runs it, or play as a guest with a seat link.',
    ),
  invalid_credentials: () => form('The username or password is wrong. Check both and try again.'),
  too_many_attempts: (error, mode) =>
    form(
      mode === 'login'
        ? `Too many failed attempts. Wait ${waitText(error.retryAfter)} before trying again.`
        : `Too many attempts to make an account from here. Wait ${waitText(error.retryAfter)} before trying again.`,
    ),
  unauthenticated: () => form('You are not signed in. Sign in and try again.'),
  forbidden_origin: () =>
    form(
      "The server does not accept requests from this web address. Open the page from the address the server's owner gave you.",
    ),
  payload_too_large: () =>
    form('That is more text than the server accepts. Use a shorter username or password.'),
  unsupported_media_type: () => form(OUT_OF_DATE),
  accounts_unavailable: () =>
    form(
      'This server has no accounts, so there is nothing to sign in to. You can still play as a guest with a seat link.',
    ),
  unavailable: () => form('The server cannot check accounts right now. Wait a moment, then try again.'),
  internal: () => form(TRY_LATER),
  unknown: (error) =>
    form(`The server refused the request, and this page does not know why. It said "${error.message}".`),
  network: () =>
    form(
      'Could not reach the server. Check your connection and that the game server is running, then try again.',
    ),
  bad_response: (error) =>
    form(
      `The server gave an answer this page could not read${error.status > 0 ? ` (status ${error.status})` : ''}. It may be down or restarting; try again in a moment.`,
    ),
}

/** What to tell a person about a failed account call; anything that is not an `AccountError` gets a general apology. */
export function describeAccountError(error: unknown, mode: AuthMode): ShownError {
  if (error instanceof AccountError) return MESSAGES[error.code](error, mode)
  return form('Something unexpected went wrong. Try again.')
}

/** The same sentence without the field, for places that have no form to attach it to (a failed sign-out). */
export function accountErrorText(error: unknown, mode: AuthMode = 'login'): string {
  return describeAccountError(error, mode).message
}
