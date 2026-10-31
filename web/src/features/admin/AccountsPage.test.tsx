// Goal: the Accounts page is for the admin alone (everyone else is told so and nothing is asked of the server), makes an account with exactly the typed username and password once, fills a random 16-character password in plain text on request, shows the new credentials once for copying without keeping them anywhere, signs nobody in, puts every refusal in plain words on the right field, lists the accounts oldest first and is reachable from the keyboard.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { flashStore } from '@/lib/flash'
import { authError, idOf } from '@/test-utils/accountsServer'
import type { AccountsScript } from '@/test-utils/accountsServer'
import { renderApp } from '@/test-utils/appHarness'
import type { App } from '@/test-utils/appHarness'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { PASSWORD_ALPHABET } from './password'

afterEach(() => {
  vi.unstubAllGlobals()
  flashStore.setState({ message: null })
  Reflect.deleteProperty(navigator, 'clipboard')
  window.localStorage.clear()
  window.sessionStorage.clear()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const BO = { username: 'Bo_new', password: 'another long passphrase' }
const users = { [ADA.username]: ADA.password, Cy_3: 'cy has a long password' }

const form = () => screen.getByRole('form', { name: 'Make an account' })
const username = () => screen.getByLabelText('Username')
const password = () => screen.getByLabelText('Password')
const create = () => within(form()).getByRole('button', { name: 'Create account' })
const makePassword = () => within(form()).getByRole('button', { name: 'Make a password' })
const sentTo = (app: App, path: string, method = 'POST') =>
  app.accounts.calls.filter((call) => call.url === path && (call.init?.method ?? 'GET') === method)

async function open(accounts: AccountsScript = {}, url = '/accounts') {
  const app = await renderApp(url, {
    accounts: { users, admin: ADA.username, signedIn: ADA.username, ...accounts },
  })
  await waitFor(() => expect(app.store.getState().status).not.toBe('loading'))
  return app
}

async function fill(app: App, name: string, secret: string) {
  if (name !== '') await app.user.type(username(), name)
  if (secret !== '') await app.user.type(password(), secret)
}

describe('the page for the admin', () => {
  it('is a titled page with the form and the list of accounts, oldest first', async () => {
    await open()
    expect(await screen.findByRole('heading', { level: 1, name: 'Accounts' })).toBeInTheDocument()
    expect(document.title).toBe('Accounts · Magic')
    expect(form()).toBeInTheDocument()
    const list = await screen.findByRole('region', { name: 'Accounts' })
    const rows = await within(list).findAllByRole('listitem')
    expect(rows.map((row) => row.querySelector('span')?.textContent)).toEqual([ADA.username, 'Cy_3'])
    expect(within(rows[0]!).getByText('Admin')).toBeInTheDocument()
    expect(within(rows[1]!).queryByText('Admin')).not.toBeInTheDocument()
    expect(within(rows[0]!).getByText(/^Made /)).toHaveAttribute('datetime', '2026-10-10T09:30:00Z')
    expect(within(list).getByText('2 accounts, oldest first.')).toBeInTheDocument()
  })

  it('asks the server for the list once and for nothing else', async () => {
    const app = await open()
    await waitFor(() => expect(app.store.getState().accounts.status).toBe('ready'))
    expect(sentTo(app, '/api/admin/users', 'GET')).toHaveLength(1)
    expect(sentTo(app, '/api/admin/users')).toHaveLength(0)
  })

  it('keeps passwords out of the address and away from password managers', async () => {
    await open()
    expect(await screen.findByRole('form', { name: 'Make an account' })).toHaveAttribute('method', 'post')
    expect(username()).toHaveAttribute('autocomplete', 'off')
    expect(password()).toHaveAttribute('autocomplete', 'off')
    expect(password()).toHaveAttribute('type', 'password')
    expect(username()).toHaveAccessibleDescription(/3 to 32 characters/)
    expect(password()).toHaveAccessibleDescription(/10 to 128 characters/)
  })

  it('shows the words of a list that did not load, and loads it again on request', async () => {
    let fail = true
    const app = await open({
      override: {
        listAccounts: () =>
          fail
            ? authError(503, 'unavailable')
            : jsonResponse(200, {
                users: [
                  { id: 'u1', username: ADA.username, created_at: '2026-10-10T09:30:00Z', is_admin: true },
                ],
              }),
      },
    })
    const alert = await screen.findByText(/Could not load the accounts/)
    expect(alert).toHaveTextContent(/cannot check accounts right now/i)
    fail = false
    await app.user.click(screen.getByRole('button', { name: 'Try again' }))
    await waitFor(() => expect(screen.queryByText(/Could not load the accounts/)).not.toBeInTheDocument())
    expect(await screen.findByText('1 account, oldest first.')).toBeInTheDocument()
  })
})

describe('everyone else', () => {
  it('is told that only the admin can make accounts, and nothing is asked of the server', async () => {
    const app = await open({ admin: 'someone_else' })
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Only the admin can make accounts.' }),
    ).toBeInTheDocument()
    expect(screen.queryByRole('form')).not.toBeInTheDocument()
    expect(sentTo(app, '/api/admin/users', 'GET')).toHaveLength(0)
    expect(sentTo(app, '/api/admin/users')).toHaveLength(0)
    expect(screen.getByRole('link', { name: 'Go home' })).toHaveAttribute('href', '/')
  })

  it('has no entry in the account menu', async () => {
    const app = await open({ admin: 'someone_else' }, '/')
    await app.user.click(screen.getByRole('button', { name: /signed in as/i }))
    expect(await screen.findByRole('menuitem', { name: 'Account settings' })).toBeInTheDocument()
    expect(screen.queryByRole('menuitem', { name: 'Accounts' })).not.toBeInTheDocument()
  })

  it('sends a visitor without a session to sign in, and back here afterwards', async () => {
    const app = await renderApp('/accounts', { accounts: { users, admin: ADA.username } })
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/login'))
    expect(app.router.state.location.search).toMatchObject({ next: '/accounts' })
    await app.user.type(screen.getByLabelText('Username'), ADA.username)
    await app.user.type(screen.getByLabelText('Password'), ADA.password)
    await app.user.click(screen.getByRole('button', { name: 'Sign in' }))
    expect(await screen.findByRole('form', { name: 'Make an account' })).toBeInTheDocument()
    expect(app.router.state.location.pathname).toBe('/accounts')
  })

  it('is told a server without accounts has none to make', async () => {
    await renderApp('/accounts', { accounts: { unavailable: true } })
    expect(
      await screen.findByRole('heading', { level: 1, name: 'This server has no accounts' }),
    ).toBeVisible()
    expect(screen.queryByRole('form')).not.toBeInTheDocument()
  })

  it('learns from the server, not from the page, when an account has stopped being the admin', async () => {
    const app = await open({ override: { createAccount: () => authError(403, 'forbidden') } })
    await fill(app, BO.username, BO.password)
    await app.user.click(create())
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Only the admin can make accounts.' }),
    ).toBeInTheDocument()
    expect(screen.queryByRole('form')).not.toBeInTheDocument()
  })
})

describe('the password generator', () => {
  it('fills a random password of 16 characters and shows it in plain text', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    expect(password()).toHaveAttribute('type', 'password')
    await app.user.click(makePassword())
    const first = (password() as HTMLInputElement).value
    expect(first).toHaveLength(16)
    for (const character of first) expect(PASSWORD_ALPHABET).toContain(character)
    expect(password()).toHaveAttribute('type', 'text')
    expect(password()).toHaveFocus()
    await app.user.click(makePassword())
    expect((password() as HTMLInputElement).value).not.toBe(first)
    expect((password() as HTMLInputElement).value).toHaveLength(16)
  })

  it('takes its randomness from crypto.getRandomValues', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    const secure = vi.spyOn(globalThis.crypto, 'getRandomValues')
    await app.user.click(makePassword())
    expect(secure).toHaveBeenCalled()
    secure.mockRestore()
  })

  it('hides and shows what is in the field on request', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    await app.user.type(password(), 'typed by the admin')
    const toggle = within(form()).getByRole('button', { name: 'Show the password' })
    expect(toggle).toHaveAttribute('aria-pressed', 'false')
    await app.user.click(toggle)
    expect(password()).toHaveAttribute('type', 'text')
    expect(password()).toHaveValue('typed by the admin')
    await app.user.click(within(form()).getByRole('button', { name: 'Hide the password' }))
    expect(password()).toHaveAttribute('type', 'password')
  })

  it('takes away the complaint about the password once there is one', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, BO.username, '')
    await app.user.click(create())
    await waitFor(() => expect(password()).toHaveAttribute('aria-invalid', 'true'))
    await app.user.click(makePassword())
    expect(password()).toHaveAttribute('aria-invalid', 'false')
  })
})

describe('making an account', () => {
  it('sends the typed username and password once, and nobody is signed in by it', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, BO.username, BO.password)
    await app.user.click(create())
    expect(await screen.findByText('Account made.')).toBeInTheDocument()
    expect(app.accounts.created).toEqual([BO])
    expect(sentTo(app, '/api/admin/users')).toHaveLength(1)
    expect(sentTo(app, '/api/auth/register')).toHaveLength(0)
    expect(sentTo(app, '/api/auth/login')).toHaveLength(0)
    expect(app.accounts.signedIn).toBe(ADA.username)
    expect(app.store.getState()).toMatchObject({ status: 'signed-in', user: { username: ADA.username } })
    expect(app.accounts.users.get(BO.username)).toBe(BO.password)
  })

  it('shows the new username and password once, with how to hand them over, and empties the fields', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    await app.user.click(makePassword())
    const generated = (password() as HTMLInputElement).value
    await app.user.type(username(), BO.username)
    await app.user.click(create())
    const shown = await screen.findByRole('region', { name: 'The new account' })
    expect(shown).toHaveTextContent(BO.username)
    expect(shown).toHaveTextContent(generated)
    expect(shown).toHaveTextContent('Tell your friend to change it under Account settings.')
    expect(username()).toHaveValue('')
    expect(password()).toHaveValue('')
    expect(password()).toHaveAttribute('type', 'password')
    expect(username()).toHaveFocus()
    expect(app.accounts.users.get(BO.username)).toBe(generated)
  })

  it('copies both lines, and removes them from the page when the admin is done', async () => {
    const writeText = vi.fn(() => Promise.resolve())
    const app = await open()
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true,
      writable: true,
    })
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, BO.username, BO.password)
    await app.user.click(create())
    await screen.findByRole('region', { name: 'The new account' })
    await app.user.click(screen.getByRole('button', { name: 'Copy the username and password' }))
    expect(writeText).toHaveBeenCalledWith(`Username: ${BO.username}\nPassword: ${BO.password}`)
    await app.user.click(screen.getByRole('button', { name: 'Done' }))
    expect(screen.queryByRole('region', { name: 'The new account' })).not.toBeInTheDocument()
    expect(document.body).not.toHaveTextContent(BO.password)
  })

  it('replaces the shown credentials when the next account is made, and drops them while it is made', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, BO.username, BO.password)
    await app.user.click(create())
    await screen.findByRole('region', { name: 'The new account' })
    await fill(app, 'Di_4', 'a different passphrase')
    await app.user.click(create())
    await waitFor(() => expect(app.accounts.created).toHaveLength(2))
    const shown = await screen.findByRole('region', { name: 'The new account' })
    expect(shown).toHaveTextContent('Di_4')
    expect(shown).not.toHaveTextContent(BO.password)
  })

  it('keeps the password nowhere in the browser', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    await app.user.click(makePassword())
    const generated = (password() as HTMLInputElement).value
    await app.user.type(username(), BO.username)
    await app.user.click(create())
    await screen.findByRole('region', { name: 'The new account' })
    const stored = JSON.stringify({ ...window.localStorage }) + JSON.stringify({ ...window.sessionStorage })
    expect(stored).not.toContain(generated)
    expect(document.cookie).not.toContain(generated)
    expect(app.router.state.location.href).not.toContain(generated)
  })

  it('lists the new account after the creation, last because it is the newest', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, BO.username, BO.password)
    await app.user.click(create())
    const list = await screen.findByRole('region', { name: 'Accounts' })
    await waitFor(() => expect(within(list).getAllByRole('listitem')).toHaveLength(3))
    const names = within(list)
      .getAllByRole('listitem')
      .map((row) => row.querySelector('span')?.textContent)
    expect(names).toEqual([ADA.username, 'Cy_3', BO.username])
  })

  it('disables the form and says what it is doing while the answer is awaited, and sends nothing twice', async () => {
    let answer!: (response: Response) => void
    const app = await open({
      override: { createAccount: () => new Promise<Response>((resolve) => (answer = resolve)) },
    })
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, BO.username, BO.password)
    await app.user.click(create())
    const busy = await screen.findByRole('button', { name: 'Making the account…' })
    expect(busy).toHaveAttribute('aria-disabled', 'true')
    expect(form()).toHaveAttribute('aria-busy', 'true')
    expect(username()).toHaveAttribute('readonly')
    await app.user.keyboard('{Enter}')
    answer(jsonResponse(201, { id: idOf(BO.username), username: BO.username }))
    expect(await screen.findByText('Account made.')).toBeInTheDocument()
    expect(sentTo(app, '/api/admin/users')).toHaveLength(1)
  })
})

describe('refusals', () => {
  it.each([
    ['nothing typed', '', '', /Enter a username/, /Enter a password/],
    ['a short username', 'ab', BO.password, /at least 3 characters/, null],
    ['a username with a space', 'new one', BO.password, /letters, digits, _ and -/, null],
    ['a short password', BO.username, 'short', null, /at least 10 characters/],
  ] as const)('checks %s before sending anything', async (_what, name, secret, nameWords, secretWords) => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, name, secret)
    await app.user.click(create())
    if (nameWords !== null) {
      await waitFor(() => expect(username()).toHaveAttribute('aria-invalid', 'true'))
      expect(username()).toHaveAccessibleDescription(nameWords)
      expect(username()).toHaveFocus()
    }
    if (secretWords !== null) {
      await waitFor(() => expect(password()).toHaveAttribute('aria-invalid', 'true'))
      expect(password()).toHaveAccessibleDescription(secretWords)
    }
    expect(sentTo(app, '/api/admin/users')).toHaveLength(0)
  })

  it('puts a taken username on the username field in the admin’s words and keeps the password', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, 'cy_3', BO.password)
    await app.user.click(create())
    await waitFor(() => expect(username()).toHaveAttribute('aria-invalid', 'true'))
    expect(username()).toHaveAccessibleDescription(/already taken.*Choose another one\. 3 to 32/)
    expect(username()).toHaveFocus()
    expect(password()).toHaveValue(BO.password)
    expect(screen.queryByRole('region', { name: 'The new account' })).not.toBeInTheDocument()
    // Changing only the name sends the account.
    await app.user.clear(username())
    await app.user.type(username(), BO.username)
    await app.user.click(create())
    expect(await screen.findByText('Account made.')).toBeInTheDocument()
    expect(app.accounts.created.at(-1)).toEqual(BO)
  })

  it.each([
    ['invalid_username', 400, 'username', /does not accept that username/],
    ['invalid_password', 400, 'password', /does not accept that password/],
  ] as const)('puts %s on the %s field', async (code, status, field, words) => {
    const app = await open({ override: { createAccount: () => authError(status, code) } })
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, BO.username, BO.password)
    await app.user.click(create())
    const input = field === 'username' ? username() : password()
    await waitFor(() => expect(input).toHaveAttribute('aria-invalid', 'true'))
    expect(input).toHaveAccessibleDescription(words)
  })

  it.each([
    ['too_many_attempts', 429, /made many accounts in a short time.*about 15 minutes/i],
    ['unavailable', 503, /cannot check accounts right now/i],
    ['internal', 500, /ran into a problem/i],
  ] as const)('shows %s as an alert about the whole form with focus on it', async (code, status, words) => {
    const app = await open({
      override: {
        createAccount: () =>
          authError(
            status,
            code,
            'refused',
            code === 'too_many_attempts' ? { 'retry-after': '900' } : undefined,
          ),
      },
    })
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, BO.username, BO.password)
    await app.user.click(create())
    const alert = await screen.findByRole('alert')
    expect(within(alert).getByText('Could not make the account')).toBeInTheDocument()
    expect(alert).toHaveTextContent(words)
    expect(alert).toHaveFocus()
    expect(username()).toHaveValue(BO.username)
    expect(password()).toHaveValue(BO.password)
  })

  it('turns to the sign-in page when the session has ended', async () => {
    const app = await open({ override: { createAccount: () => authError(401, 'unauthenticated') } })
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, BO.username, BO.password)
    await app.user.click(create())
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/login'))
  })
})

describe('the keyboard', () => {
  it('reaches every control of the form in reading order', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    username().focus()
    const order: (string | null)[] = []
    for (let step = 0; step < 5; step += 1) {
      order.push(document.activeElement?.getAttribute('id') ?? document.activeElement?.textContent ?? null)
      await app.user.tab()
    }
    expect(order).toEqual([
      'made-username',
      'made-password',
      'Make a password',
      'Show the password',
      'Create account',
    ])
  })

  it('makes the account when Enter is pressed in a field', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Make an account' })
    await fill(app, BO.username, BO.password)
    await app.user.keyboard('{Enter}')
    expect(await screen.findByText('Account made.')).toBeInTheDocument()
    expect(app.accounts.created).toEqual([BO])
  })
})
