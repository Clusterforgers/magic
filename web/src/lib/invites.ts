/**
 * The seat links of the game this tab created, kept in `sessionStorage` so the host can hand them out again from the
 * lobby after leaving the create page (the server shows a token only once). Every access can fail (blocked storage,
 * private windows, a full quota) and the app works without it.
 */
import { isCount, isListOf, isRecord, isText } from '@/net/check'

/** One seat of a created game, with its token. */
export interface Invite {
  seat: number
  name: string
  token: string
}

/** All the seats of one created game. */
export interface Invites {
  game: string
  seats: Invite[]
}

/** The part of `Storage` used here; tests pass their own. */
export interface KeyValueStore {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

const KEY = 'magic.invites'

function sessionStore(): KeyValueStore | null {
  try {
    return window.sessionStorage
  } catch {
    return null
  }
}

const isInvite = (value: unknown): value is Invite =>
  isRecord(value) && isCount(value.seat) && isText(value.name) && isText(value.token)

/** Remembers the invites of the newest game made in this tab; false when the browser refused. */
export function saveInvites(invites: Invites, store: KeyValueStore | null = sessionStore()): boolean {
  try {
    store?.setItem(KEY, JSON.stringify(invites))
    return store !== null
  } catch {
    return false
  }
}

/** The remembered invites if they belong to `game`; null when there are none or they cannot be read. */
export function loadInvites(game: string, store: KeyValueStore | null = sessionStore()): Invites | null {
  try {
    const text = store?.getItem(KEY)
    if (text === null || text === undefined) return null
    const value: unknown = JSON.parse(text)
    if (!isRecord(value) || value.game !== game || !isListOf(value.seats, isInvite)) return null
    return { game, seats: value.seats }
  } catch {
    return null
  }
}
