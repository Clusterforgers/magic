// Goal: the sign-in store moves between loading, signed in, signed out and unavailable exactly as the server's answers say, a failed sign-in never lets anyone in, a sign-out clears the user and the games, answers for an old question never overwrite a newer state, a failed game leaves the list when the server removes it and stays when it cannot, no password is ever kept, the server is asked once whether registering is open, and the admin's list and form follow the admin flag the server gives and take it back when the server does.
import { describe, expect, it, vi } from 'vitest'
import { AccountError } from '@/net/accounts'
import { authError, idOf, listed, startAccounts } from '@/test-utils/accountsServer'
import type { AccountsScript } from '@/test-utils/accountsServer'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { createAuthStore, NO_ACCOUNTS, NO_GAMES, offlineAccounts } from './store'

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const NEW = { username: 'Bo_new', password: 'another long passphrase' }

function setup(script: AccountsScript = {}) {
  const server = startAccounts({ users: { [ADA.username]: ADA.password }, ...script })
  const store = createAuthStore({ api: server.api })
  return { server, store, state: () => store.getState() }
}

/** A promise settled by hand, to hold an answer back while something else happens. */
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

describe('starting', () => {
  it('begins loading, with nobody signed in and no games', () => {
    const { state } = setup()
    expect(state()).toMatchObject({ status: 'loading', user: null, games: NO_GAMES, notice: null })
  })

  it('becomes signed in when the server knows the cookie', async () => {
    const { state, server } = setup({ signedIn: ADA.username })
    await state().start()
    expect(state()).toMatchObject({
      status: 'signed-in',
      user: { id: idOf(ADA.username), username: ADA.username },
    })
    expect(server.calls.map((call) => call.url)).toEqual(['/api/auth/options', '/api/auth/me'])
    expect(server.calls[0]?.init?.credentials).toBe('include')
  })

  it('becomes signed out on a 401', async () => {
    const { state } = setup()
    await state().start()
    expect(state()).toMatchObject({ status: 'signed-out', user: null })
  })

  it('becomes unavailable when the server has no accounts, and stays a guest app', async () => {
    const { state } = setup({ unavailable: true })
    await state().start()
    expect(state()).toMatchObject({ status: 'unavailable', user: null, games: NO_GAMES })
  })

  it.each([
    ['the network fails', () => Promise.reject(new TypeError('Failed to fetch'))],
    ['the server fails', () => authError(500, 'internal')],
    ['the database is down', () => authError(503, 'unavailable')],
    ['a proxy answers with HTML', () => new Response('<html>', { status: 502 })],
    ['the answer is not an account', () => jsonResponse(200, { nope: true })],
  ])('becomes signed out when %s, since nothing is known to be signed in', async (_what, answer) => {
    const { state } = setup({ signedIn: ADA.username, override: { me: answer } })
    await state().start()
    expect(state()).toMatchObject({ status: 'signed-out', user: null })
  })

  it('asks the server once, however many times it is called (React mounts twice)', async () => {
    const { state, server } = setup({ signedIn: ADA.username })
    await Promise.all([state().start(), state().start()])
    await state().start()
    expect(server.calls.map((call) => call.url)).toEqual(['/api/auth/options', '/api/auth/me'])
  })

  it('asks nothing when the store was given its state', async () => {
    const server = startAccounts()
    const store = createAuthStore({ api: server.api, initial: { status: 'signed-out' } })
    await store.getState().start()
    expect(server.calls).toHaveLength(0)
    expect(store.getState().status).toBe('signed-out')
  })

  it('ignores a who-am-I answer that arrives after a sign-in made it out of date', async () => {
    const held = deferred<Response>()
    const { state } = setup({ override: { me: () => held.promise } })
    const starting = state().start()
    await state().login(ADA)
    expect(state().status).toBe('signed-in')
    held.resolve(authError(401, 'unauthenticated'))
    await starting
    expect(state()).toMatchObject({ status: 'signed-in', user: { username: ADA.username } })
  })

  it('has no accounts, and asks for nothing, in the stand-in used when a page has no provider', async () => {
    const store = createAuthStore({ api: offlineAccounts, initial: { status: 'unavailable' } })
    await store.getState().start()
    expect(store.getState().status).toBe('unavailable')
    const result = await store.getState().login(ADA)
    expect(result).toMatchObject({ ok: false, error: { code: 'accounts_unavailable' } })
  })
})

describe('registering', () => {
  it('makes the account and signs it in', async () => {
    const { state, server } = setup()
    await state().start()
    const result = await state().register(NEW)
    expect(result).toMatchObject({ ok: true, user: { username: NEW.username } })
    expect(state()).toMatchObject({ status: 'signed-in', user: { username: NEW.username } })
    expect(server.signedIn).toBe(NEW.username)
    expect(server.calls.at(-1)?.init?.credentials).toBe('include')
  })

  it.each([
    [
      'the username is taken (ignoring case)',
      { username: 'ada_l', password: NEW.password },
      'username_taken',
    ],
    ['the username is not allowed', { username: 'a b', password: NEW.password }, 'invalid_username'],
    ['the password is not allowed', { username: NEW.username, password: 'short' }, 'invalid_password'],
  ])('keeps the visitor out when %s, and returns the error', async (_what, credentials, code) => {
    const { state } = setup()
    await state().start()
    const result = await state().register(credentials)
    expect(result).toMatchObject({ ok: false, error: { code } })
    expect(state()).toMatchObject({ status: 'signed-out', user: null })
  })

  it('keeps the visitor out when registration is closed', async () => {
    const { state } = setup({ closed: true })
    await state().start()
    const result = await state().register(NEW)
    expect(result).toMatchObject({ ok: false, error: { code: 'registration_closed' } })
    expect(state().status).toBe('signed-out')
  })

  it('becomes unavailable when the server turns out to have no accounts', async () => {
    const server = startAccounts({ unavailable: true })
    const store = createAuthStore({ api: server.api, initial: { status: 'signed-out' } })
    const result = await store.getState().register(NEW)
    expect(result).toMatchObject({ ok: false, error: { code: 'accounts_unavailable' } })
    expect(store.getState().status).toBe('unavailable')
  })
})

describe('signing in', () => {
  it('signs in with the right credentials and returns the account', async () => {
    const { state, server } = setup()
    await state().start()
    const result = await state().login(ADA)
    expect(result).toEqual({
      ok: true,
      user: { id: idOf(ADA.username), username: ADA.username, is_admin: false },
    })
    expect(state()).toMatchObject({ status: 'signed-in', user: { username: ADA.username } })
    expect(server.credentials).toEqual([ADA])
  })

  it.each([
    ['a wrong password', { username: ADA.username, password: 'not the right one!' }],
    ['an unknown username', { username: 'nobody', password: ADA.password }],
    ['a username that breaks the rules', { username: 'no body', password: ADA.password }],
  ])('keeps the visitor out with %s, with one and the same error', async (_what, credentials) => {
    const { state } = setup()
    await state().start()
    const result = await state().login(credentials)
    expect(result).toMatchObject({ ok: false, error: { code: 'invalid_credentials', status: 401 } })
    expect(state()).toMatchObject({ status: 'signed-out', user: null, games: NO_GAMES })
  })

  it('keeps the visitor out and passes on the wait when the server throttles', async () => {
    const { state } = setup({ throttleAfter: 2 })
    await state().start()
    for (let i = 0; i < 2; i += 1)
      await state().login({ username: ADA.username, password: 'not the right one!' })
    const result = await state().login(ADA)
    expect(result).toMatchObject({ ok: false, error: { code: 'too_many_attempts', retryAfter: 900 } })
    expect(state().status).toBe('signed-out')
  })

  it.each([
    ['the network fails', () => Promise.reject(new TypeError('Failed to fetch')), 'network'],
    ['the server fails', () => authError(500, 'internal'), 'internal'],
  ])('keeps the visitor out when %s', async (_what, answer, code) => {
    const { state } = setup({ override: { login: answer } })
    await state().start()
    const result = await state().login(ADA)
    expect(result).toMatchObject({ ok: false, error: { code } })
    expect(state()).toMatchObject({ status: 'signed-out', user: null })
  })

  it('leaves a signed-in account as it was when another sign-in fails', async () => {
    const { state } = setup({ signedIn: ADA.username })
    await state().start()
    await state().login({ username: ADA.username, password: 'not the right one!' })
    expect(state()).toMatchObject({ status: 'signed-in', user: { username: ADA.username } })
  })

  it('replaces the user, and forgets the previous user’s games, when another account signs in', async () => {
    const bo = { username: 'Bo', password: 'bo has a long password' }
    const { state } = setup({
      users: { [ADA.username]: ADA.password, [bo.username]: bo.password },
      signedIn: ADA.username,
      games: [listed()],
    })
    await state().start()
    await state().loadGames()
    expect(state().games.games).toHaveLength(1)
    await state().login(bo)
    expect(state()).toMatchObject({ status: 'signed-in', user: { username: 'Bo' }, games: NO_GAMES })
  })
})

describe('passwords', () => {
  const secret = 'zebra-battery-staple-9'

  it('are in no state after register or login, successful or not', async () => {
    const { state } = setup()
    await state().start()
    await state().register({ username: NEW.username, password: secret }) // works
    await state().register({ username: NEW.username, password: secret }) // taken
    await state().login({ username: 'nobody', password: secret }) // refused
    await state().logout()
    await state().login({ username: NEW.username, password: secret }) // works
    expect(state().status).toBe('signed-in')
    const kept = JSON.stringify(state(), (_key, value: unknown) =>
      typeof value === 'function' ? undefined : value,
    )
    expect(kept).not.toContain(secret)
  })

  it('are not in the error a failed sign-in returns', async () => {
    const { state } = setup()
    await state().start()
    const result = await state().login({ username: ADA.username, password: secret })
    expect(result.ok).toBe(false)
    if (result.ok || !(result.error instanceof AccountError)) throw new Error('expected an AccountError')
    expect(
      `${JSON.stringify(result.error)} ${result.error.message} ${String(result.error.stack)}`,
    ).not.toContain(secret)
  })
})

describe('signing out', () => {
  it('clears the user, the games and any notice, and tells the server', async () => {
    const { state, server } = setup({ signedIn: ADA.username, games: [listed(), listed({ game: 'other' })] })
    await state().start()
    await state().loadGames()
    expect(state().games).toMatchObject({ status: 'ready' })
    expect(await state().logout()).toBe(true)
    expect(state()).toMatchObject({ status: 'signed-out', user: null, games: NO_GAMES, notice: null })
    expect(server.signedIn).toBeNull()
    expect(server.calls.at(-1)).toMatchObject({
      url: '/api/auth/logout',
      init: { method: 'POST', credentials: 'include' },
    })
  })

  it('stays signed in, and says so, when the server could not be told', async () => {
    const { state } = setup({
      signedIn: ADA.username,
      games: [listed()],
      override: { logout: () => Promise.reject(new TypeError('Failed to fetch')) },
    })
    await state().start()
    await state().loadGames()
    expect(await state().logout()).toBe(false)
    expect(state()).toMatchObject({ status: 'signed-in', user: { username: ADA.username } })
    expect(state().games.games).toHaveLength(1)
    expect(state().notice).toMatch(/could not sign out.*could not reach the server/i)
    state().dismissNotice()
    expect(state().notice).toBeNull()
  })

  it('asks the server for nothing when nobody is signed in', async () => {
    const { state, server } = setup()
    await state().start()
    expect(await state().logout()).toBe(true)
    expect(server.calls.map((call) => call.url)).toEqual(['/api/auth/options', '/api/auth/me'])
  })

  it('drops a game list that was still on its way', async () => {
    const held = deferred<Response>()
    const { state } = setup({ signedIn: ADA.username, override: { games: () => held.promise } })
    await state().start()
    const loading = state().loadGames()
    expect(state().games.status).toBe('loading')
    await state().logout()
    held.resolve(jsonResponse(200, { games: [listed()] }))
    await loading
    expect(state()).toMatchObject({ status: 'signed-out', games: NO_GAMES })
  })
})

describe('loading the games', () => {
  it('lists them newest first whatever order the server used', async () => {
    const old = listed({ game: 'old', created_at: '2026-10-17T09:00:00Z' })
    const middle = listed({ game: 'middle', created_at: '2026-10-18T09:00:00Z' })
    const newest = listed({ game: 'new', created_at: '2026-10-18T18:30:00Z' })
    const { state } = setup({ signedIn: ADA.username, games: [middle, old, newest] })
    await state().start()
    await state().loadGames()
    expect(state().games).toMatchObject({ status: 'ready', error: null })
    expect(state().games.games.map((game) => game.game)).toEqual(['new', 'middle', 'old'])
  })

  it('says loading while it asks, and keeps the older list meanwhile', async () => {
    let next: Promise<Response> | null = null
    const { state } = setup({
      signedIn: ADA.username,
      override: { games: () => next ?? jsonResponse(200, { games: [listed()] }) },
    })
    await state().start()
    await state().loadGames()
    const later = deferred<Response>()
    next = later.promise
    const loading = state().loadGames()
    expect(state().games).toMatchObject({ status: 'loading', error: null })
    expect(state().games.games).toHaveLength(1)
    later.resolve(jsonResponse(200, { games: [listed(), listed({ game: 'second' })] }))
    await loading
    expect(state().games).toMatchObject({ status: 'ready' })
    expect(state().games.games).toHaveLength(2)
  })

  it('does nothing unless signed in', async () => {
    const { state, server } = setup()
    await state().start()
    await state().loadGames()
    expect(server.calls.map((call) => call.url)).toEqual(['/api/auth/options', '/api/auth/me'])
    expect(state().games).toEqual(NO_GAMES)
  })

  it('fails with words, keeps the list it had, and works again on the next try', async () => {
    let broken = false
    const { state } = setup({
      signedIn: ADA.username,
      override: {
        games: () => (broken ? authError(503, 'unavailable') : jsonResponse(200, { games: [listed()] })),
      },
    })
    await state().start()
    await state().loadGames()
    broken = true
    await state().loadGames()
    expect(state().games.status).toBe('failed')
    expect(state().games.error).toMatch(/cannot check accounts right now/i)
    expect(state().games.games).toHaveLength(1)
    broken = false
    await state().loadGames()
    expect(state().games).toMatchObject({ status: 'ready', error: null })
  })

  it('turns signed out when the session has ended, which the server reports as 401', async () => {
    const { state, server } = setup({ signedIn: ADA.username, games: [listed()] })
    await state().start()
    await state().loadGames()
    await server.api.logout() // the cookie goes: expired, or signed out in another tab
    await state().loadGames()
    expect(state()).toMatchObject({ status: 'signed-out', user: null, games: NO_GAMES })
  })

  it('turns unavailable when the server says it has no accounts', async () => {
    const { state } = setup({
      signedIn: ADA.username,
      override: { games: () => authError(503, 'accounts_unavailable') },
    })
    await state().start()
    await state().loadGames()
    expect(state()).toMatchObject({ status: 'unavailable', user: null, games: NO_GAMES })
  })

  it('uses the newest answer when two loads overlap, whichever arrives last', async () => {
    const first = deferred<Response>()
    const second = deferred<Response>()
    const queue = [first, second]
    const { state } = setup({
      signedIn: ADA.username,
      override: { games: () => queue.shift()?.promise ?? authError(500, 'internal') },
    })
    await state().start()
    const one = state().loadGames()
    const two = state().loadGames()
    second.resolve(jsonResponse(200, { games: [listed({ game: 'fresh' })] }))
    await two
    first.resolve(jsonResponse(200, { games: [listed({ game: 'stale' })] }))
    await one
    expect(state().games.games.map((game) => game.game)).toEqual(['fresh'])
  })
})

describe('removing a failed game', () => {
  const failed = listed({ game: 'broken', status: 'failed', reason: 'The deck of Bo has changed.' })
  const running = listed({ game: 'live', status: 'running' })

  async function loaded(script: AccountsScript = {}) {
    const made = setup({ signedIn: ADA.username, games: [failed, running], ...script })
    await made.state().start()
    await made.state().loadGames()
    return made
  }
  const gameIds = (state: () => { games: { games: readonly { game: string }[] } }): string[] =>
    state().games.games.map((one) => one.game)

  it('asks the server and drops the row at once, leaving the others', async () => {
    const { state, server } = await loaded()
    await expect(state().removeGame('broken')).resolves.toEqual({ ok: true })
    expect(gameIds(state)).toEqual(['live'])
    const last = server.calls.at(-1)
    expect([last?.url, last?.init?.method]).toEqual(['/api/me/games/broken', 'DELETE'])
    expect(state().games.status).toBe('ready')
  })

  it('keeps the row and says why when the server fails', async () => {
    const { state } = await loaded({ override: { removeGame: () => authError(503, 'unavailable') } })
    const result = await state().removeGame('broken')
    expect(result).toMatchObject({ ok: false, error: { code: 'unavailable', status: 503 } })
    expect(gameIds(state)).toEqual(['broken', 'live'])
  })

  it('loads the list again when the server holds no such game, which then shows what is true', async () => {
    const { state, server } = await loaded({
      override: { removeGame: () => jsonResponse(404, { error: 'not_found', message: 'gone' }) },
    })
    server.setGames([running])
    await expect(state().removeGame('broken')).resolves.toEqual({ ok: true })
    expect(gameIds(state)).toEqual(['live'])
    expect(server.calls.filter((call) => call.url === '/api/me/games')).toHaveLength(2)
  })

  it('turns signed out when the session has ended', async () => {
    const { state } = await loaded({ override: { removeGame: () => authError(401, 'unauthenticated') } })
    const result = await state().removeGame('broken')
    expect(result).toMatchObject({ ok: false })
    expect(state()).toMatchObject({ status: 'signed-out', user: null, games: NO_GAMES })
  })

  it('does nothing unless signed in', async () => {
    const { state, server } = setup()
    await state().start()
    const result = await state().removeGame('broken')
    expect(result).toMatchObject({ ok: false, error: { code: 'unauthenticated' } })
    expect(server.calls.map((call) => call.url)).toEqual(['/api/auth/options', '/api/auth/me'])
  })
})

describe('the registration option', () => {
  it.each([
    ['open', {}, 'open'],
    ['closed', { closed: true }, 'closed'],
  ] as const)('reads %s from the server when it starts', async (_what, script, expected) => {
    const { state } = setup(script)
    expect(state().registration).toBe('unknown')
    await state().start()
    expect(state().registration).toBe(expected)
  })

  it('stays unknown, and the form stays, when the server does not answer', async () => {
    const { state } = setup({ override: { options: () => authError(500, 'internal') } })
    await state().start()
    expect(state()).toMatchObject({ status: 'signed-out', registration: 'unknown' })
  })

  it('is read for a signed-in account too, without holding the sign-in back', async () => {
    const { state } = setup({ signedIn: ADA.username, closed: true })
    await state().start()
    expect(state()).toMatchObject({ status: 'signed-in', registration: 'closed' })
  })
})

describe("the admin's accounts", () => {
  const BO = { username: 'Bo_new', password: 'another long passphrase' }

  async function asAdmin(script: AccountsScript = {}) {
    const made = setup({ admin: ADA.username, signedIn: ADA.username, closed: true, ...script })
    await made.state().start()
    return made
  }

  it('carries the admin flag from who-am-I, and only for the admin', async () => {
    const admin = await asAdmin()
    expect(admin.state().user).toMatchObject({ username: ADA.username, is_admin: true })
    const other = setup({ admin: 'someone_else', signedIn: ADA.username })
    await other.state().start()
    expect(other.state().user).toMatchObject({ is_admin: false })
  })

  it('lists the accounts, oldest first, for the admin', async () => {
    const { state } = await asAdmin()
    await state().loadAccounts()
    expect(state().accounts.status).toBe('ready')
    expect(state().accounts.users.map((one) => one.username)).toEqual([ADA.username])
    expect(state().accounts.users[0]).toMatchObject({ is_admin: true })
  })

  it('asks the server for nothing unless the signed-in account is the admin', async () => {
    const { state, server } = setup({ admin: 'someone_else', signedIn: ADA.username })
    await state().start()
    const before = server.calls.length
    await state().loadAccounts()
    const result = await state().createAccount(BO)
    expect(result).toMatchObject({ ok: false, error: { code: 'forbidden' } })
    expect(server.calls).toHaveLength(before)
    expect(state().accounts).toEqual(NO_ACCOUNTS)
  })

  it('makes an account without signing it in, then reloads the list', async () => {
    const { state, server } = await asAdmin()
    await state().loadAccounts()
    const result = await state().createAccount(BO)
    expect(result).toEqual({ ok: true, account: { id: idOf(BO.username), username: BO.username } })
    expect(server.created).toEqual([BO])
    expect(server.signedIn).toBe(ADA.username)
    expect(state()).toMatchObject({ status: 'signed-in', user: { username: ADA.username } })
    await vi.waitFor(() =>
      expect(state().accounts.users.map((one) => one.username)).toEqual([ADA.username, BO.username]),
    )
  })

  it.each([
    ['a taken username', 409, 'username_taken'],
    ['a username the server refuses', 400, 'invalid_username'],
    ['a throttled admin', 429, 'too_many_attempts'],
    ['a database that is down', 503, 'unavailable'],
  ] as const)('keeps everything as it was after %s', async (_what, status, code) => {
    const { state } = await asAdmin({ override: { createAccount: () => authError(status, code) } })
    const result = await state().createAccount(BO)
    expect(result).toMatchObject({ ok: false, error: { code, status } })
    expect(state()).toMatchObject({ status: 'signed-in', user: { is_admin: true } })
  })

  it('stops counting the account as the admin when the server says forbidden', async () => {
    const { state } = await asAdmin({ override: { createAccount: () => authError(403, 'forbidden') } })
    await state().loadAccounts()
    const result = await state().createAccount(BO)
    expect(result).toMatchObject({ ok: false, error: { code: 'forbidden' } })
    expect(state()).toMatchObject({ status: 'signed-in', user: { username: ADA.username, is_admin: false } })
    expect(state().accounts).toEqual(NO_ACCOUNTS)
  })

  it('turns signed out when the session has ended, on a list as on a creation', async () => {
    const lost = await asAdmin({ override: { listAccounts: () => authError(401, 'unauthenticated') } })
    await lost.state().loadAccounts()
    expect(lost.state()).toMatchObject({ status: 'signed-out', user: null, accounts: NO_ACCOUNTS })
    const gone = await asAdmin({ override: { createAccount: () => authError(401, 'unauthenticated') } })
    await gone.state().createAccount(BO)
    expect(gone.state()).toMatchObject({ status: 'signed-out', user: null })
  })

  it('keeps the last list and says why when a reload fails', async () => {
    const row = { id: 'u-1', username: ADA.username, created_at: '2026-10-10T09:30:00Z', is_admin: true }
    let asked = 0
    const { state } = await asAdmin({
      override: {
        listAccounts: () =>
          (asked += 1) === 1 ? jsonResponse(200, { users: [row] }) : authError(503, 'unavailable'),
      },
    })
    await state().loadAccounts()
    expect(state().accounts.status).toBe('ready')
    await state().loadAccounts()
    expect(state().accounts.status).toBe('failed')
    expect(state().accounts.users).toHaveLength(1)
    expect(state().accounts.error).toMatch(/cannot check accounts right now/i)
  })

  it('drops a list that was still on its way when someone else signs in', async () => {
    const late = deferred<Response>()
    const { state } = await asAdmin({ override: { listAccounts: () => late.promise } })
    const loading = state().loadAccounts()
    await state().logout()
    late.resolve(jsonResponse(200, { users: [] }))
    await loading
    expect(state().accounts).toEqual(NO_ACCOUNTS)
  })

  describe("a friend's account", () => {
    const FRIEND = { username: 'Cy_3', password: 'cy has a long password' }
    const friends = { [ADA.username]: ADA.password, [FRIEND.username]: FRIEND.password }

    it('sets a password the admin chose and tells the server nothing else', async () => {
      const { state, server } = await asAdmin({ users: friends })
      await state().loadAccounts()
      const result = await state().resetAccountPassword(idOf(FRIEND.username), 'a fresh passphrase')
      expect(result).toEqual({ ok: true })
      expect(server.resets).toEqual([{ id: idOf(FRIEND.username), password: 'a fresh passphrase' }])
      expect(server.users.get(FRIEND.username)).toBe('a fresh passphrase')
      expect(server.signedIn).toBe(ADA.username)
      expect(JSON.stringify(state())).not.toContain('a fresh passphrase')
    })

    it('removes the account, drops the row at once and reloads the list', async () => {
      const { state, server } = await asAdmin({ users: friends })
      await state().loadAccounts()
      expect(state().accounts.users).toHaveLength(2)
      const result = await state().removeAccount(idOf(FRIEND.username))
      expect(result).toEqual({ ok: true })
      expect(server.removed).toEqual([idOf(FRIEND.username)])
      expect(state().accounts.users.map((one) => one.username)).toEqual([ADA.username])
      await vi.waitFor(() => expect(state().accounts.status).toBe('ready'))
      expect(state().accounts.users.map((one) => one.username)).toEqual([ADA.username])
    })

    it('asks the server for nothing unless the signed-in account is the admin', async () => {
      const { state, server } = setup({ admin: 'someone_else', signedIn: ADA.username })
      await state().start()
      const before = server.calls.length
      expect(await state().resetAccountPassword('id-x', 'a fresh passphrase')).toMatchObject({
        ok: false,
        error: { code: 'forbidden' },
      })
      expect(await state().removeAccount('id-x')).toMatchObject({ ok: false, error: { code: 'forbidden' } })
      expect(server.calls).toHaveLength(before)
    })

    it.each([
      ['a password the rules refuse', 400, 'invalid_password'],
      ["the admin's own account", 409, 'admin_account'],
      ['a throttled admin', 429, 'too_many_attempts'],
      ['a database that is down', 503, 'unavailable'],
    ] as const)('keeps everything as it was after %s', async (_what, status, code) => {
      const refuse = () => authError(status, code)
      const { state } = await asAdmin({
        users: friends,
        override: { resetPassword: refuse, removeAccount: refuse },
      })
      await state().loadAccounts()
      const reset = await state().resetAccountPassword(idOf(FRIEND.username), 'a fresh passphrase')
      expect(reset).toMatchObject({ ok: false, error: { code, status } })
      const removal = await state().removeAccount(idOf(FRIEND.username))
      expect(removal).toMatchObject({ ok: false, error: { code, status } })
      expect(state()).toMatchObject({ status: 'signed-in', user: { is_admin: true } })
      expect(state().accounts.users).toHaveLength(2)
    })

    it('reloads the list when the account is already gone, and counts a removal of it as done', async () => {
      const gone = () => authError(404, 'unknown_account')
      const { state, server } = await asAdmin({
        users: friends,
        override: { resetPassword: gone, removeAccount: gone },
      })
      await state().loadAccounts()
      const listings = () => server.calls.filter((call) => call.url === '/api/admin/users').length
      const before = listings()
      const reset = await state().resetAccountPassword(idOf(FRIEND.username), 'a fresh passphrase')
      expect(reset).toMatchObject({ ok: false, error: { code: 'unknown_account' } })
      const removal = await state().removeAccount(idOf(FRIEND.username))
      expect(removal).toEqual({ ok: true })
      await vi.waitFor(() => expect(listings()).toBe(before + 2))
    })

    it('stops counting the account as the admin on forbidden and turns signed out on unauthenticated', async () => {
      const demoted = await asAdmin({
        users: friends,
        override: { removeAccount: () => authError(403, 'forbidden') },
      })
      await demoted.state().removeAccount(idOf(FRIEND.username))
      expect(demoted.state()).toMatchObject({ status: 'signed-in', user: { is_admin: false } })
      const lost = await asAdmin({
        users: friends,
        override: { resetPassword: () => authError(401, 'unauthenticated') },
      })
      await lost.state().resetAccountPassword(idOf(FRIEND.username), 'a fresh passphrase')
      expect(lost.state()).toMatchObject({ status: 'signed-out', user: null })
    })
  })

  it('clears the list and the flag with the sign-out', async () => {
    const { state } = await asAdmin()
    await state().loadAccounts()
    await state().logout()
    expect(state()).toMatchObject({ status: 'signed-out', user: null, accounts: NO_ACCOUNTS })
  })
})
