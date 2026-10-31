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
import type { AccountRow, CreatedAccount, Credentials, MyGame, UserView } from '@/protocol/generated'
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

/** The admin's list of accounts as the Accounts page sees it. */
export interface AccountsState {
  /** `idle` before the first load, `loading` while one is on its way, `failed` after a load that did not work. */
  status: 'idle' | 'loading' | 'ready' | 'failed'
  /** The last list that loaded, oldest first; it stays while the next load runs or fails. */
  users: readonly AccountRow[]
  /** Why the last load failed, in words for the person, or null. */
  error: string | null
}

/** Whether the server takes new accounts, as far as this browser has been told; `unknown` until it has been asked. */
export type RegistrationState = 'unknown' | 'open' | 'closed'

/** What `register` and `login` give back; a failure keeps the state as it was. */
export type AuthResult = { ok: true; user: UserView } | { ok: false; error: unknown }

/** What `createAccount` gives back; a failure keeps the state as it was. */
export type CreateResult = { ok: true; account: CreatedAccount } | { ok: false; error: unknown }

/** What `changePassword` and `deleteAccount` give back; a failure keeps the state as it was. */
export type SettingsResult = { ok: true } | { ok: false; error: unknown }

/** Everything the pages read, and the actions they call. */
export interface AuthState {
  status: AuthStatus
  /** The account when `status` is `signed-in`, else null. */
  user: UserView | null
  /** The user's games; reset whenever who is signed in changes. */
  games: GamesState
  /** The admin's list of accounts; reset whenever who is signed in changes. */
  accounts: AccountsState
  /** Whether the server takes new accounts; the sign-up screen says so instead of failing after the form is sent. */
  registration: RegistrationState
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
  /** Replaces the password. The server ends the account's other sessions; this one stays signed in. */
  changePassword(current: string, next: string): Promise<SettingsResult>
  /** Deletes the account, confirmed with its password. On success nobody is signed in any more. */
  deleteAccount(password: string): Promise<SettingsResult>
  /** Loads the user's games (and reloads them); does nothing unless signed in. */
  loadGames(): Promise<void>
  /** Loads the admin's list of accounts (and reloads it); does nothing unless the signed-in account is the admin. */
  loadAccounts(): Promise<void>
  /**
   * Makes an account for someone else; nobody is signed in by it. The list reloads on success. A 403 means the
   * server no longer counts this account as the admin, and the page stops offering the form.
   */
  createAccount(credentials: Credentials): Promise<CreateResult>
  /**
   * Sets the password of an account that is not the admin's and ends its sessions; admin only. The password is not
   * kept. A 404 reloads the list, since the account is gone.
   */
  resetAccountPassword(id: string, password: string): Promise<SettingsResult>
  /**
   * Removes an account that is not the admin's, with its decks; admin only. The list reloads; an account that is
   * already gone counts as removed.
   */
  removeAccount(id: string): Promise<SettingsResult>
  /**
   * Takes a failed game off the user's list. The row goes at once on success; when the server says it holds no such
   * game for this user, the list is loaded again and shows what is true.
   */
  removeGame(game: string): Promise<SettingsResult>
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

/** No accounts listed, and none asked for. */
export const NO_ACCOUNTS: AccountsState = { status: 'idle', users: [], error: null }

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
  let listRound = 0
  let asked: Promise<void> | null = null

  return createStore<AuthState>()((set, get) => {
    function become(status: AuthStatus, user: UserView | null): void {
      round += 1
      listRound += 1
      set({ status, user, games: NO_GAMES, accounts: NO_ACCOUNTS, notice: null })
    }

    // The server says this account is not the admin (any more): the page stops offering what only the admin can do.
    function demoted(): void {
      listRound += 1
      set((state) => ({
        user: state.user === null ? null : { ...state.user, is_admin: false },
        accounts: NO_ACCOUNTS,
      }))
    }

    // The server says it has no accounts: whatever was believed before is moot.
    const unavailable = (error: unknown): boolean => {
      if (!isCode(error, 'accounts_unavailable')) return false
      become('unavailable', null)
      return true
    }

    // What an admin call that failed says about who is signed in, shared by the three calls on accounts.
    function adminFailed(error: unknown): void {
      if (isCode(error, 'unauthenticated')) become('signed-out', null)
      else if (isCode(error, 'forbidden')) demoted()
      else unavailable(error)
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
      accounts: NO_ACCOUNTS,
      registration: 'unknown',
      notice: null,

      start() {
        if (get().status !== 'loading') return Promise.resolve()
        asked ??= (async () => {
          const before = round
          // Asked together with who is signed in. An answer that does not come leaves the form in place: the server
          // judges the registration itself.
          const options = api.registrationOptions().then(
            (answer) => set({ registration: answer.registration_open ? 'open' : 'closed' }),
            () => undefined,
          )
          try {
            const user = await api.me()
            if (before === round) become('signed-in', user)
          } catch (error) {
            if (before !== round) return
            // Not signed in, or the server could not say (down, restarting): either way nothing is known to be
            // signed in, and the next sign-in or a reload finds out the rest.
            become(isCode(error, 'accounts_unavailable') ? 'unavailable' : 'signed-out', null)
          } finally {
            await options
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

      async changePassword(current, next) {
        try {
          await api.changePassword({ current_password: current, new_password: next })
        } catch (error) {
          // A session that ended elsewhere, or a server that has no accounts, changes who this page thinks is here.
          if (isCode(error, 'unauthenticated')) become('signed-out', null)
          else unavailable(error)
          return { ok: false, error }
        }
        return { ok: true }
      },

      async deleteAccount(password) {
        try {
          await api.deleteAccount({ password })
        } catch (error) {
          if (isCode(error, 'unauthenticated')) become('signed-out', null)
          else unavailable(error)
          return { ok: false, error }
        }
        become('signed-out', null)
        return { ok: true }
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

      async loadAccounts() {
        if (get().status !== 'signed-in' || get().user?.is_admin !== true) return
        listRound += 1
        const mine = listRound
        set((state) => ({ accounts: { ...state.accounts, status: 'loading', error: null } }))
        try {
          const users = await api.listAccounts()
          if (mine === listRound) set({ accounts: { status: 'ready', users, error: null } })
        } catch (error) {
          if (mine !== listRound) return
          if (isCode(error, 'unauthenticated')) become('signed-out', null)
          else if (isCode(error, 'forbidden')) demoted()
          else if (!unavailable(error)) {
            set((state) => ({
              accounts: { status: 'failed', users: state.accounts.users, error: accountErrorText(error) },
            }))
          }
        }
      },

      async createAccount(credentials) {
        if (get().status !== 'signed-in' || get().user?.is_admin !== true) {
          return { ok: false, error: NOT_ADMIN }
        }
        try {
          const account = await api.createAccount(credentials)
          void get().loadAccounts()
          return { ok: true, account }
        } catch (error) {
          adminFailed(error)
          return { ok: false, error }
        }
      },

      async resetAccountPassword(id, password) {
        if (get().status !== 'signed-in' || get().user?.is_admin !== true) {
          return { ok: false, error: NOT_ADMIN }
        }
        try {
          await api.resetPassword(id, { password })
          return { ok: true }
        } catch (error) {
          adminFailed(error)
          // An account that is gone shows as gone: the list loads again.
          if (isCode(error, 'unknown_account')) void get().loadAccounts()
          return { ok: false, error }
        }
      },

      async removeAccount(id) {
        if (get().status !== 'signed-in' || get().user?.is_admin !== true) {
          return { ok: false, error: NOT_ADMIN }
        }
        try {
          await api.removeAccount(id)
        } catch (error) {
          adminFailed(error)
          // Already gone is what was asked for; the list loads again and shows what is true.
          if (isCode(error, 'unknown_account')) {
            void get().loadAccounts()
            return { ok: true }
          }
          return { ok: false, error }
        }
        // The row goes at once; the reload confirms what the server holds.
        set((state) => ({
          accounts: { ...state.accounts, users: state.accounts.users.filter((row) => row.id !== id) },
        }))
        void get().loadAccounts()
        return { ok: true }
      },

      async removeGame(game) {
        if (get().status !== 'signed-in') return { ok: false, error: NOT_SIGNED_IN }
        try {
          await api.removeGame(game)
        } catch (error) {
          if (isCode(error, 'unauthenticated')) become('signed-out', null)
          else if (!unavailable(error) && error instanceof AccountError && error.status === 404) {
            await get().loadGames()
            return { ok: true }
          }
          return { ok: false, error }
        }
        set((state) => ({
          games: { ...state.games, games: state.games.games.filter((one) => one.game !== game) },
        }))
        return { ok: true }
      },

      dismissNotice: () => set({ notice: null }),
    }
  })
}

const OFFLINE = new AccountError('accounts_unavailable', 503, 'This page has no accounts.')
const NOT_SIGNED_IN = new AccountError('unauthenticated', 401, 'Sign in to do that.')
const NOT_ADMIN = new AccountError('forbidden', 403, 'Only the admin can do that.')

/** A client that has no accounts: what a page without a provider (a test) talks to. */
export const offlineAccounts: AccountsApi = {
  register: () => Promise.reject(OFFLINE),
  login: () => Promise.reject(OFFLINE),
  logout: () => Promise.reject(OFFLINE),
  me: () => Promise.reject(OFFLINE),
  registrationOptions: () => Promise.reject(OFFLINE),
  listAccounts: () => Promise.reject(OFFLINE),
  createAccount: () => Promise.reject(OFFLINE),
  resetPassword: () => Promise.reject(OFFLINE),
  removeAccount: () => Promise.reject(OFFLINE),
  myGames: () => Promise.reject(OFFLINE),
  removeGame: () => Promise.reject(OFFLINE),
  changePassword: () => Promise.reject(OFFLINE),
  deleteAccount: () => Promise.reject(OFFLINE),
}
