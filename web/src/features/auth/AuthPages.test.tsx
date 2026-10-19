// Goal: the sign-in and register pages show the server's rules, check them before sending, send exactly the typed credentials once, put every failure in plain words on the right field with focus on the first problem, empty the password after every submit, and follow only safe `next` addresses afterwards.
import { act, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { authError, startAccounts } from '@/test-utils/accountsServer'
import type { AccountsScript } from '@/test-utils/accountsServer'
import { renderApp } from '@/test-utils/appHarness'
import type { App } from '@/test-utils/appHarness'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { renderInRouter } from '@/test-utils/router'
import { AuthProvider } from './AuthProvider'
import { AuthScreen } from './AuthScreen'
import { createAuthStore } from './store'

afterEach(() => {
  vi.unstubAllGlobals()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const users = { [ADA.username]: ADA.password }

const username = () => screen.getByLabelText('Username')
const password = () => screen.getByLabelText('Password')
const form = () => screen.getByRole('form')
const submitButton = (name: string | RegExp) => screen.getByRole('button', { name })
const here = (app: App) => app.router.state.location.pathname

/** Opens a sign-in page and waits until the page has asked who is signed in. */
async function open(url: string, accounts: AccountsScript = {}, strict = false) {
  const app = await renderApp(url, { accounts: { users, ...accounts }, strict })
  await screen.findByRole('form')
  await waitFor(() => expect(app.store.getState().status).not.toBe('loading'))
  return app
}

async function type(app: App, name: string, secret: string) {
  if (name !== '') await app.user.type(username(), name)
  if (secret !== '') await app.user.type(password(), secret)
}

const sentTo = (app: App, path: string) => app.accounts.calls.filter((call) => call.url === path)

describe.each([
  ['/login', 'Sign in', 'current-password', 'Sign in', 'Create one', '/register'],
  ['/register', 'Create an account', 'new-password', 'Create account', 'Sign in', '/login'],
])('the page at %s', (url, heading, passwordComplete, submit, otherLink, otherPath) => {
  it('is a form named for what it does, in a titled page', async () => {
    await open(url)
    expect(screen.getByRole('form', { name: heading })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 1, name: heading })).toBeInTheDocument()
    expect(document.title).toBe(`${heading} · Magic`)
    expect(submitButton(submit)).toBeEnabled()
    expect(submitButton(submit)).toHaveAttribute('type', 'submit')
  })

  it('would never put the credentials in the address, even if a submit got past the page’s handler', async () => {
    await open(url)
    expect(form()).toHaveAttribute('method', 'post')
  })

  it('has a username field and a password field that password managers recognise', async () => {
    await open(url)
    expect(username()).toHaveAttribute('autocomplete', 'username')
    expect(username()).toHaveAttribute('name', 'username')
    expect(username()).toHaveAttribute('autocapitalize', 'none')
    expect(username()).toHaveAttribute('spellcheck', 'false')
    expect(password()).toHaveAttribute('type', 'password')
    expect(password()).toHaveAttribute('autocomplete', passwordComplete)
    expect(password()).toHaveAttribute('name', 'password')
  })

  it('states the rules next to the fields and ties them to the inputs', async () => {
    await open(url)
    expect(username()).toHaveAccessibleDescription(/3 to 32 characters: letters, digits, _ and -/)
    expect(password()).toHaveAccessibleDescription(/10 to 128 characters/)
  })

  it('links to the other form and takes the way on with it', async () => {
    await open(`${url}?next=/games`)
    const link = within(form()).getByRole('link', { name: otherLink })
    expect(link.getAttribute('href')).toMatch(new RegExp(`^${otherPath}\\?next=(%2F|/)games$`))
  })

  it('links to the other form without a next when there is none', async () => {
    await open(url)
    expect(within(form()).getByRole('link', { name: otherLink })).toHaveAttribute('href', otherPath)
  })

  it('has the header’s links to both forms for a visitor who is not signed in', async () => {
    await open(url)
    const nav = screen.getByRole('navigation', { name: 'Account' })
    expect(within(nav).getByRole('link', { name: 'Sign in' })).toHaveAttribute('href', '/login')
    expect(within(nav).getByRole('link', { name: 'Register' })).toHaveAttribute('href', '/register')
  })
})

describe.each(['/login', '/register'])('checking before sending, at %s', (url) => {
  const cases: [string, string, string, string | RegExp, 'username' | 'password'][] = [
    ['nothing typed', '', '', /enter a username/i, 'username'],
    [
      'a username that is too short',
      'ab',
      'correct horse battery',
      /at least 3 characters \(this one has 2\)/,
      'username',
    ],
    [
      'a username with a space',
      'ada l',
      'correct horse battery',
      /only letters, digits, _ and -/,
      'username',
    ],
    [
      'a username with an at sign',
      'ada@mail',
      'correct horse battery',
      /only letters, digits, _ and -/,
      'username',
    ],
    [
      'a username that is too long',
      'a'.repeat(33),
      'correct horse battery',
      /at most 32 characters/,
      'username',
    ],
    ['no password', 'ada_l', '', /enter a password/i, 'password'],
    [
      'a password that is too short',
      'ada_l',
      'too short',
      /at least 10 characters \(this one has 9\)/,
      'password',
    ],
    [
      'a password that is too long',
      'ada_l',
      'x'.repeat(129),
      /at most 128 characters \(this one has 129\)/,
      'password',
    ],
  ]

  it.each(cases)(
    'sends nothing for %s, and focuses the first wrong field',
    async (_what, name, secret, words, field) => {
      const app = await open(url)
      await type(app, name, secret)
      await app.user.click(submitButton(/sign in|create account/i))
      const wrong = field === 'username' ? username() : password()
      expect(wrong).toHaveAttribute('aria-invalid', 'true')
      expect(wrong).toHaveAccessibleDescription(words)
      expect(wrong).toHaveFocus()
      expect(sentTo(app, '/api/auth/login')).toHaveLength(0)
      expect(sentTo(app, '/api/auth/register')).toHaveLength(0)
      expect(submitButton(/sign in|create account/i)).toBeEnabled()
    },
  )

  it('focuses the username first when both fields are wrong, and marks both', async () => {
    const app = await open(url)
    await app.user.click(submitButton(/sign in|create account/i))
    expect(username()).toHaveAttribute('aria-invalid', 'true')
    expect(password()).toHaveAttribute('aria-invalid', 'true')
    expect(username()).toHaveFocus()
  })

  it('clears a problem as soon as the field is fixed, and keeps what was typed', async () => {
    const app = await open(url)
    await type(app, 'ab', 'correct horse battery')
    await app.user.click(submitButton(/sign in|create account/i))
    expect(username()).toHaveAttribute('aria-invalid', 'true')
    await app.user.type(username(), 'c')
    expect(username()).toHaveAttribute('aria-invalid', 'false')
    expect(username()).toHaveValue('abc')
    expect(password()).toHaveValue('correct horse battery')
    expect(username()).not.toHaveAccessibleDescription(/at least 3/)
  })

  it('accepts a password of ten spaces and sends it exactly, since nothing is trimmed', async () => {
    const app = await open(url)
    await type(app, 'fresh_one', '          ')
    await app.user.click(submitButton(/sign in|create account/i))
    await waitFor(() => expect(app.accounts.credentials).toHaveLength(1))
    expect(app.accounts.credentials).toEqual([{ username: 'fresh_one', password: '          ' }])
  })
})

describe('signing in', () => {
  it('sends the typed credentials once, untrimmed, then goes home signed in', async () => {
    const app = await open('/login')
    await type(app, ADA.username, ADA.password)
    await app.user.click(submitButton('Sign in'))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-in'))
    expect(app.accounts.credentials).toEqual([ADA])
    await waitFor(() => expect(here(app)).toBe('/'))
    expect(await screen.findByRole('heading', { level: 1, name: 'Play' })).toBeInTheDocument()
    const nav = screen.getByRole('navigation', { name: 'Account' })
    expect(within(nav).getByText(ADA.username)).toBeInTheDocument()
    expect(app.router.state.location.href).toBe('/')
  })

  it('does not leave the sign-in page in the history, so Back does not return to the form', async () => {
    const app = await open('/login')
    await type(app, ADA.username, ADA.password)
    await app.user.click(submitButton('Sign in'))
    await waitFor(() => expect(here(app)).toBe('/'))
    app.router.history.back()
    await app.settle()
    expect(here(app)).not.toBe('/login')
  })

  it('submits with Enter in the password field', async () => {
    const app = await open('/login')
    await type(app, ADA.username, `${ADA.password}{Enter}`)
    await waitFor(() => expect(app.store.getState().status).toBe('signed-in'))
    expect(sentTo(app, '/api/auth/login')).toHaveLength(1)
  })

  it('disables the button and says what it is doing while the answer is awaited', async () => {
    let answer!: (response: Response) => void
    const app = await open('/login', {
      override: { login: () => new Promise<Response>((resolve) => (answer = resolve)) },
    })
    await type(app, ADA.username, ADA.password)
    await app.user.click(submitButton('Sign in'))
    const busy = await screen.findByRole('button', { name: 'Signing in…' })
    expect(busy).toHaveAttribute('aria-disabled', 'true')
    expect(form()).toHaveAttribute('aria-busy', 'true')
    expect(username()).toHaveAttribute('readonly')
    // A second press, or Enter, sends nothing more.
    await app.user.keyboard('{Enter}')
    expect(sentTo(app, '/api/auth/login')).toHaveLength(1)
    answer(jsonResponse(200, { id: 'id-ada', username: ADA.username }))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-in'))
    expect(sentTo(app, '/api/auth/login')).toHaveLength(1)
  })

  const WRONG = 'not the right one!'
  const failures: [string, { username: string; password: string }, AccountsScript, string | RegExp][] = [
    ['a wrong password', { username: ADA.username, password: WRONG }, {}, /username or password is wrong/i],
    [
      'an unknown username',
      { username: 'nobody_here', password: ADA.password },
      {},
      /username or password is wrong/i,
    ],
    ['too many attempts', ADA, { throttleAfter: 0 }, /too many failed attempts\. wait about 15 minutes/i],
    [
      'a network failure',
      ADA,
      { override: { login: () => Promise.reject(new TypeError('x')) } },
      /could not reach the server/i,
    ],
    ['a server error', ADA, { override: { login: () => authError(500, 'internal') } }, /ran into a problem/i],
    [
      'an answer that is not JSON',
      ADA,
      { override: { login: () => new Response('<html>', { status: 502 }) } },
      /status 502/,
    ],
  ]

  it.each(failures)(
    'explains %s in the form’s alert, focuses it, and stays out',
    async (_what, typed, script, words) => {
      const app = await open('/login', script)
      await type(app, typed.username, typed.password)
      await app.user.click(submitButton('Sign in'))
      const alert = await screen.findByRole('alert')
      expect(alert).toHaveTextContent(words)
      expect(alert).toHaveFocus()
      expect(app.store.getState()).toMatchObject({ status: 'signed-out', user: null })
      expect(here(app)).toBe('/login')
      expect(submitButton('Sign in')).toBeEnabled()
    },
  )

  it('says the same words for a wrong password and an unknown username', async () => {
    const app = await open('/login')
    await type(app, ADA.username, WRONG)
    await app.user.click(submitButton('Sign in'))
    const first = (await screen.findByRole('alert')).textContent
    await app.user.clear(username())
    await app.user.type(username(), 'nobody_here')
    await app.user.type(password(), ADA.password)
    await app.user.click(submitButton('Sign in'))
    await waitFor(() => expect(sentTo(app, '/api/auth/login')).toHaveLength(2))
    const second = (await screen.findByRole('alert')).textContent
    expect(second).toBe(first)
  })

  it('empties the password after a failure but keeps the username, and the next try works', async () => {
    const app = await open('/login')
    await type(app, ADA.username, 'not the right one!')
    await app.user.click(submitButton('Sign in'))
    await screen.findByRole('alert')
    expect(password()).toHaveValue('')
    expect(username()).toHaveValue(ADA.username)
    await app.user.type(password(), ADA.password)
    await app.user.click(submitButton('Sign in'))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-in'))
    expect(sentTo(app, '/api/auth/login')).toHaveLength(2)
  })

  it('keeps the password in no state, and in no attribute, after it was sent', async () => {
    const app = await open('/login')
    const secret = 'zebra-battery-staple-9'
    await type(app, ADA.username, secret)
    await app.user.click(submitButton('Sign in'))
    await screen.findByRole('alert')
    expect(password()).toHaveValue('')
    expect(app.container.innerHTML).not.toContain(secret)
    const kept = JSON.stringify(app.store.getState(), (_key, value: unknown) =>
      typeof value === 'function' ? undefined : value,
    )
    expect(kept).not.toContain(secret)
  })

  it('shows a server without accounts as a note instead of a form', async () => {
    const app = await renderApp('/login', { accounts: { unavailable: true } })
    expect(await screen.findByRole('heading', { name: 'This server has no accounts' })).toBeInTheDocument()
    expect(screen.queryByRole('form')).toBeNull()
    expect(screen.getByRole('link', { name: 'Create a game' })).toHaveAttribute('href', '/')
    expect(screen.queryByRole('navigation', { name: 'Account' })).toBeNull()
    expect(app.store.getState().status).toBe('unavailable')
  })

  it('finds out during the submit that the server has no accounts, and says so', async () => {
    const app = await open('/login', { override: { login: () => authError(503, 'accounts_unavailable') } })
    await type(app, ADA.username, ADA.password)
    await app.user.click(submitButton('Sign in'))
    expect(await screen.findByRole('heading', { name: 'This server has no accounts' })).toBeInTheDocument()
    expect(app.store.getState().status).toBe('unavailable')
  })

  it('sends a visitor who is already signed in on, without showing the form', async () => {
    const app = await renderApp('/login?next=/games', { accounts: { users, signedIn: ADA.username } })
    await waitFor(() => expect(here(app)).toBe('/games'))
    expect(screen.queryByRole('form', { name: 'Sign in' })).toBeNull()
  })
})

describe('registering', () => {
  const NEW = { username: 'Bo_new', password: 'another long passphrase' }

  it('makes the account, signs it in and goes home', async () => {
    const app = await open('/register')
    await type(app, NEW.username, NEW.password)
    await app.user.click(submitButton('Create account'))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-in'))
    expect(app.accounts.credentials).toEqual([NEW])
    expect(app.accounts.signedIn).toBe(NEW.username)
    await waitFor(() => expect(here(app)).toBe('/'))
    expect(
      within(screen.getByRole('navigation', { name: 'Account' })).getByText(NEW.username),
    ).toBeInTheDocument()
  })

  it('says a taken username is taken, on the username field, with focus there', async () => {
    const app = await open('/register')
    await type(app, 'ada_l', NEW.password) // the same name in other case
    await app.user.click(submitButton('Create account'))
    await waitFor(() => expect(username()).toHaveAttribute('aria-invalid', 'true'))
    expect(username()).toHaveAccessibleDescription(/already taken.*upper and lower case/i)
    expect(username()).toHaveFocus()
    expect(screen.queryByRole('alert')).toBeNull()
    expect(password()).toHaveValue('')
    expect(app.store.getState().status).toBe('signed-out')
  })

  it('clears the taken message once the username is edited', async () => {
    const app = await open('/register')
    await type(app, 'ada_l', NEW.password)
    await app.user.click(submitButton('Create account'))
    await waitFor(() => expect(username()).toHaveAttribute('aria-invalid', 'true'))
    await app.user.type(username(), '2')
    expect(username()).toHaveAttribute('aria-invalid', 'false')
  })

  it('says a closed server takes no new accounts', async () => {
    const app = await open('/register', { closed: true })
    await type(app, NEW.username, NEW.password)
    await app.user.click(submitButton('Create account'))
    expect(await screen.findByRole('alert')).toHaveTextContent(/not taking new accounts.*guest/i)
    expect(screen.getByRole('alert')).toHaveFocus()
  })

  it('puts a password the server refuses on the password field', async () => {
    const app = await open('/register', { override: { register: () => authError(400, 'invalid_password') } })
    await type(app, NEW.username, NEW.password)
    await app.user.click(submitButton('Create account'))
    await waitFor(() => expect(password()).toHaveAttribute('aria-invalid', 'true'))
    expect(password()).toHaveFocus()
    expect(password()).toHaveAccessibleDescription(/10 to 128 characters/)
  })
})

describe('the next address', () => {
  // What the page itself acts on: the address’s search after `validateSearch`, not the raw one.
  const searchOf = (app: App): unknown => app.router.state.matches.at(-1)?.search

  it.each([
    ['/login?next=/games', ADA, '/games'],
    ['/login?next=%2Fgames', ADA, '/games'],
    ['/register?next=/games', { username: 'Bo_new', password: 'another long passphrase' }, '/games'],
    ['/login?next=/game/abc', ADA, '/game/abc'],
    ['/login', ADA, '/'],
  ])('%s leads to %s after signing in', async (url, credentials, destination) => {
    const app = await open(url)
    await type(app, credentials.username, credentials.password)
    await app.user.click(submitButton(/sign in|create account/i))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-in'))
    await waitFor(() => expect(here(app)).toBe(destination))
  })

  it.each([
    ['//evil.example', '/login?next=//evil.example'],
    ['https://evil.example', '/login?next=https://evil.example'],
    ['https:%2F%2Fevil.example', '/login?next=https%3A%2F%2Fevil.example'],
    ['a protocol-relative address, encoded', '/login?next=%2F%2Fevil.example'],
    ['a backslash trick', '/login?next=%2F%5Cevil.example'],
    ['a tab trick', '/login?next=%2F%09%2Fevil.example'],
    ['a javascript address', '/login?next=javascript:alert(1)'],
    ['a number', '/login?next=5'],
    ['the sign-in page itself', '/login?next=/login'],
    ['the register page', '/login?next=/register'],
  ])('ignores %s and goes home instead', async (_what, url) => {
    const app = await open(url)
    expect(searchOf(app)).toEqual({})
    await type(app, ADA.username, ADA.password)
    await app.user.click(submitButton('Sign in'))
    await waitFor(() => expect(app.store.getState().status).toBe('signed-in'))
    await waitFor(() => expect(here(app)).toBe('/'))
    expect(app.router.state.location.href).toBe('/')
  })

  it('never keeps a fragment (a seat token) from next', async () => {
    const app = await open('/login?next=%2Fgame%2Fabc%23seat%3D1%26token%3Dsecret')
    expect(searchOf(app)).toEqual({ next: '/game/abc' })
    await type(app, ADA.username, ADA.password)
    await app.user.click(submitButton('Sign in'))
    await waitFor(() => expect(here(app)).toBe('/game/abc'))
    expect(app.router.state.location.href).not.toContain('secret')
  })
})

describe('the screen on its own', () => {
  it.each(['https://evil.example/steal', '//evil.example', '/\\evil.example', 'javascript:alert(1)'])(
    'does not go to %j when a router hands it over unchecked',
    async (unchecked) => {
      const errors = vi.spyOn(console, 'error').mockImplementation(() => undefined)
      const server = startAccounts({ users, signedIn: ADA.username })
      const store = createAuthStore({ api: server.api })
      const { router } = await renderInRouter(
        <AuthProvider store={store}>
          <AuthScreen mode="login" next={unchecked} />
        </AuthProvider>,
      )
      await waitFor(() => expect(store.getState().status).toBe('signed-in'))
      await act(async () => {
        await Promise.resolve()
      })
      // A jump to another site would be a full page load, which jsdom reports as an error.
      expect(errors).not.toHaveBeenCalled()
      expect(router.state.location.href).toBe('/')
      errors.mockRestore()
    },
  )
})

describe('mounting twice', () => {
  it('asks the server who is signed in once, however React mounts the page', async () => {
    const app = await open('/login', {}, true)
    expect(sentTo(app, '/api/auth/me')).toHaveLength(1)
    expect(screen.getAllByRole('form')).toHaveLength(1)
  })
})
