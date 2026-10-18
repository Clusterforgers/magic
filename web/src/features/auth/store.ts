/**
 * The signed-in state of the app, in a zustand store: who the server says this browser is, the user's list of games,
 * and the actions that change them. No React; `context.tsx` hands the store to the pages.
 *
 * The session itself is an `HttpOnly` cookie the page cannot read, so the truth is whatever `GET /api/auth/me` says.
 * Passwords pass through `register` and `login` into the request and are kept nowhere.
 */
import { createStore } from 'zustand/vanilla'
import type { StoreApi } from 'zustand/vanilla'
import { AccountError, accountsApi } from '@/net/accounts'
import type { AccountsApi } from '@/net/accounts'
import type { Credentials, MyGame, UserView } from '@/protocol/generated'
import { accountErrorText } from './errors'

/**
 * Where sign-in stands. `loading`: the first `GET /api/auth/me` has not answered. `unavailable`: the server has no
 * accounts (503 `accounts_unavailable`), so everyone plays as a guest and nothing offers a sign-in.
 */
export type AuthStatus = 'loading' | 'signed-out' | 'signed-in' | 'unavailable'

/** The user's games as the pages see them. */
export interface GamesState {
  /** `idle` before the first load, `loading` while one is on its way, `failed` after a load that did not work. */
  status: 'idle' | 'loading' | 'ready' | 'failed'
  /** The last list that loaded, newest first; it stays while the next load runs or fails. */
  games: readonly MyGame[]
  /** Why the last load failed, in words for the person, or null. */
  error: string | null
}

/** What `register` and `login` give back; a failure keeps the state as it was. */
export type AuthResult = { ok: true; user: UserView } | { ok: false; error: unknown }

/** Everything the pages read, and the actions they call. */
export interface AuthState {
  status: AuthStatus
  /** The account when `status` is `signed-in`, else null. */
  user: UserView | null
  /** The user's games; reset whenever who is signed in changes. */
  games: GamesState
  /** A problem with no form to show it (a sign-out that failed), or null; clear it with `dismissNotice`. */
  notice: string | null
  /** Asks the server who this is. Runs once, however often it is called. */
  start(): Promise<void>
  /** Makes an account and signs it in. */
  register(credentials: Credentials): Promise<AuthResult>
  /** Signs in. */
  login(credentials: Credentials): Promise<AuthResult>
  /** Signs out; false, with a `notice`, when the server could not be told. Clears the user and the games. */
  logout(): Promise<boolean>
  /** Loads the user's games (and reloads them); does nothing unless signed in. */
  loadGames(): Promise<void>
  /** Clears the notice. */
  dismissNotice(): void
}

/** The store type: a vanilla zustand store; in React read it with `useAuth(selector)`. */
export type AuthStore = StoreApi<AuthState>

/** What `createAuthStore` can be told. */
export interface AuthDeps {
  /** The account calls; the shared client by default. */
  api?: AccountsApi
  /** Start somewhere other than `loading` (tests; and the stand-in store of a page that has no provider). */
  initial?: { status: AuthStatus; user?: UserView | null }
}

/** No games, and none asked for. */
export const NO_GAMES: GamesState = { status: 'idle', games: [], error: null }

const isCode = (error: unknown, code: AccountError['code']): boolean =>
  error instanceof AccountError && error.code === code

// The server lists newest first; sorting again makes the pages independent of that. Equal times keep their order.
function newestFirst(games: readonly MyGame[]): MyGame[] {
  return [...games].sort((a, b) => (a.created_at < b.created_at ? 1 : a.created_at > b.created_at ? -1 : 0))
}

/** Builds a store. Call it once for the app (`appAuth`); a test builds its own with a fake `api`. */
export function createAuthStore({ api = accountsApi, initial }: AuthDeps = {}): AuthStore {
  // Bumped whenever who is signed in changes or a list is asked for again, so an answer that was on its way for
  // someone else, or for an older question, is dropped when it arrives.
  let round = 0
  let asked: Promise<void> | null = null

  return createStore<AuthState>()((set, get) => {
    function become(status: AuthStatus, user: UserView | null): void {
      round += 1
      set({ status, user, games: NO_GAMES, notice: null })
    }

    // The server says it has no accounts: whatever was believed before is moot.
    const unavailable = (error: unknown): boolean => {
      if (!isCode(error, 'accounts_unavailable')) return false
      become('unavailable', null)
      return true
    }

    async function enter(call: () => Promise<UserView>): Promise<AuthResult> {
      try {
        const user = await call()
        become('signed-in', user)
        return { ok: true, user }
      } catch (error) {
        unavailable(error)
        return { ok: false, error }
      }
    }

    return {
      status: initial?.status ?? 'loading',
      user: initial?.user ?? null,
      games: NO_GAMES,
      notice: null,

      start() {
        if (get().status !== 'loading') return Promise.resolve()
        asked ??= (async () => {
          const before = round
          try {
            const user = await api.me()
            if (before === round) become('signed-in', user)
          } catch (error) {
            if (before !== round) return
            // Not signed in, or the server could not say (down, restarting): either way nothing is known to be
            // signed in, and the next sign-in or a reload finds out the rest.
            become(isCode(error, 'accounts_unavailable') ? 'unavailable' : 'signed-out', null)
          }
        })()
        return asked
      },

      register: (credentials) => enter(() => api.register(credentials)),
      login: (credentials) => enter(() => api.login(credentials)),

      async logout() {
        if (get().status !== 'signed-in') return true
        try {
          await api.logout()
        } catch (error) {
          if (unavailable(error)) return true
          set({ notice: `Could not sign out. ${accountErrorText(error)}` })
          return false
        }
        become('signed-out', null)
        return true
      },

      async loadGames() {
        if (get().status !== 'signed-in') return
        round += 1
        const mine = round
        set((state) => ({ games: { ...state.games, status: 'loading', error: null } }))
        try {
          const games = newestFirst(await api.myGames())
          if (mine === round) set({ games: { status: 'ready', games, error: null } })
        } catch (error) {
          if (mine !== round) return
          if (isCode(error, 'unauthenticated')) become('signed-out', null)
          else if (!unavailable(error)) {
            set((state) => ({
              games: { status: 'failed', games: state.games.games, error: accountErrorText(error) },
            }))
          }
        }
      },

      dismissNotice: () => set({ notice: null }),
    }
  })
}

const OFFLINE = new AccountError('accounts_unavailable', 503, 'This page has no accounts.')

/** A client that has no accounts: what a page without a provider (a test) talks to. */
export const offlineAccounts: AccountsApi = {
  register: () => Promise.reject(OFFLINE),
  login: () => Promise.reject(OFFLINE),
  logout: () => Promise.reject(OFFLINE),
  me: () => Promise.reject(OFFLINE),
  myGames: () => Promise.reject(OFFLINE),
}
