/**
 * The account preference endpoints of `mtg-server` (docs/server.md, Account preferences): read the pace preferences of
 * the signed-in account and replace them. The answer is the generated `UserPrefs`, checked at runtime; a failure
 * rejects with `PrefsError`. Every request carries the session cookie (`credentials: 'include'`).
 */
import type { Stop, UserPrefs } from '@/protocol/generated'
import type { Fetch } from './api'
import { isFlag, isListOf, isRecord } from './check'

/** A preference call that did not work: the server refused it, could not be reached or answered something unreadable. */
export class PrefsError extends Error {
  readonly status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = 'PrefsError'
    this.status = status
  }
}

/** The preference calls. */
export interface PrefsApi {
  /** `GET /api/me/prefs`. */
  load(): Promise<UserPrefs>
  /** `PUT /api/me/prefs`: replaces the preferences; resolves with them as stored. */
  save(prefs: UserPrefs): Promise<UserPrefs>
}

/** Options for `createPrefsApi`. */
export interface PrefsOptions {
  /** Defaults to the global `fetch`, looked up at call time. */
  fetch?: Fetch
  /** Prefix for every path; empty means the page origin. */
  baseUrl?: string
}

function isStop(value: unknown): value is Stop {
  return (
    isRecord(value) &&
    (value.side === 'Own' || value.side === 'Opponent') &&
    typeof value.step === 'string' &&
    isFlag(value.keep)
  )
}

function readPrefs(body: unknown): UserPrefs | null {
  if (!isRecord(body) || !isListOf(body.default_stops, isStop) || !isFlag(body.end_turn_by_hand)) return null
  return { default_stops: body.default_stops, end_turn_by_hand: body.end_turn_by_hand }
}

/** Builds the client; call it once, or use the shared `prefsApi`. */
export function createPrefsApi(options: PrefsOptions = {}): PrefsApi {
  const url = `${options.baseUrl ?? ''}/api/me/prefs`

  async function call(init: RequestInit): Promise<UserPrefs> {
    const send: Fetch = options.fetch ?? ((target, request) => globalThis.fetch(target, request))
    let response: Response
    let body: unknown
    try {
      response = await send(url, { credentials: 'include', ...init })
      body = await response.json().catch(() => undefined)
    } catch {
      throw new PrefsError('Could not reach the server.', 0)
    }
    if (!response.ok)
      throw new PrefsError(`The server answered with HTTP ${response.status}.`, response.status)
    const prefs = readPrefs(body)
    if (prefs === null)
      throw new PrefsError('The server sent an answer this page cannot read.', response.status)
    return prefs
  }

  return {
    load: () => call({}),
    save: (prefs) =>
      call({ method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(prefs) }),
  }
}

/** The client for the page origin, using the global `fetch`. */
export const prefsApi: PrefsApi = createPrefsApi()
