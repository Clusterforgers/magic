// Goal: the admin sets a new password for a friend (the same generator, the credentials shown once with the address and nothing kept) and removes an account after a question that says what goes, from the rows of the Accounts list; the admin's own row offers neither, every refusal is in plain words, a gone account reloads the list, the controls are reachable from the keyboard and focus goes back where the admin was.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { authError, idOf } from '@/test-utils/accountsServer'
import type { AccountsScript } from '@/test-utils/accountsServer'
import { renderApp } from '@/test-utils/appHarness'
import type { App } from '@/test-utils/appHarness'
import { PASSWORD_ALPHABET } from './password'

afterEach(() => {
  vi.unstubAllGlobals()
  Reflect.deleteProperty(navigator, 'clipboard')
  Reflect.deleteProperty(Element.prototype, 'scrollIntoView')
  window.localStorage.clear()
  window.sessionStorage.clear()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const CY = { username: 'Cy_3', password: 'cy has a long password' }
const users = { [ADA.username]: ADA.password, [CY.username]: CY.password, Di_4: 'di has a long password' }
const FRESH = 'a fresh passphrase'

const newPasswordButton = (name = CY.username) =>
  screen.findByRole('button', { name: `New password for ${name}` })
const removeButton = (name = CY.username) => screen.findByRole('button', { name: `Remove ${name}` })
const resetForm = (name = CY.username) => screen.getByRole('form', { name: `New password for ${name}` })
const passwordField = () => within(resetForm()).getByLabelText('New password')
const sentTo = (app: App, path: string, method: string) =>
  app.accounts.calls.filter((call) => call.url === path && (call.init?.method ?? 'GET') === method)
const listings = (app: App) => sentTo(app, '/api/admin/users', 'GET').length

async function open(accounts: AccountsScript = {}) {
  const app = await renderApp('/accounts', {
    accounts: { users, admin: ADA.username, signedIn: ADA.username, ...accounts },
  })
  await waitFor(() => expect(app.store.getState().status).not.toBe('loading'))
  await newPasswordButton()
  return app
}

function rowOf(name: string): HTMLElement {
  const list = screen.getByRole('region', { name: 'Accounts' })
  const row = within(list)
    .getAllByRole('listitem')
    .find((item) => item.querySelector('span')?.textContent === name)
  if (row === undefined) throw new Error(`no row for ${name}`)
  return row
}

function clipboard() {
  const writeText = vi.fn(() => Promise.resolve())
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true, writable: true })
  return writeText
}

describe('the row of a friend', () => {
  it('offers a new password and a removal, named for the account, and the admin’s own row offers neither', async () => {
    await open()
    for (const name of [CY.username, 'Di_4']) {
      const row = rowOf(name)
      expect(within(row).getByRole('button', { name: `New password for ${name}` })).toHaveTextContent(
        'New password',
      )
      expect(within(row).getByRole('button', { name: `Remove ${name}` })).toHaveTextContent('Remove')
    }
    expect(within(rowOf(ADA.username)).queryByRole('button')).not.toBeInTheDocument()
  })

  it('reaches both actions in reading order from the keyboard', async () => {
    const app = await open()
    ;(await newPasswordButton()).focus()
    await app.user.tab()
    expect(await removeButton()).toHaveFocus()
    await app.user.tab()
    expect(await newPasswordButton('Di_4')).toHaveFocus()
  })
})

describe('a new password', () => {
  it('opens a form with the generator under the row, focus on the field and a password manager left out', async () => {
    const app = await open()
    const trigger = await newPasswordButton()
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    await app.user.click(trigger)
    expect(trigger).toHaveAttribute('aria-expanded', 'true')
    expect(rowOf(CY.username)).toContainElement(resetForm())
    expect(passwordField()).toHaveFocus()
    expect(passwordField()).toHaveAttribute('type', 'password')
    expect(passwordField()).toHaveAttribute('autocomplete', 'new-password')
    expect(passwordField()).toHaveAttribute('data-1p-ignore')
    expect(passwordField()).toHaveAccessibleDescription(/10 to 128 characters/)
    expect(resetForm()).toHaveAttribute('method', 'post')

    await app.user.click(within(resetForm()).getByRole('button', { name: 'Make a password' }))
    const generated = (passwordField() as HTMLInputElement).value
    expect(generated).toHaveLength(16)
    for (const character of generated) expect(PASSWORD_ALPHABET).toContain(character)
    expect(passwordField()).toHaveAttribute('type', 'text')
    expect(sentTo(app, `/api/admin/users/${idOf(CY.username)}/password`, 'POST')).toHaveLength(0)
  })

  it('sends the chosen password once, then shows the address, name and password once, with focus on Copy both', async () => {
    const scroll = vi.fn()
    Object.defineProperty(Element.prototype, 'scrollIntoView', {
      value: scroll,
      configurable: true,
      writable: true,
    })
    const app = await open()
    // After the page is open: user-event installs a clipboard of its own when it is set up.
    const writeText = clipboard()
    await app.user.click(await newPasswordButton())
    await app.user.click(within(resetForm()).getByRole('button', { name: 'Make a password' }))
    const generated = (passwordField() as HTMLInputElement).value
    await app.user.click(within(resetForm()).getByRole('button', { name: 'Set password' }))

    const shown = await screen.findByRole('region', { name: `The new password for ${CY.username}` })
    expect(app.accounts.resets).toEqual([{ id: idOf(CY.username), password: generated }])
    expect(sentTo(app, `/api/admin/users/${idOf(CY.username)}/password`, 'POST')).toHaveLength(1)
    expect(app.accounts.users.get(CY.username)).toBe(generated)
    expect(app.accounts.signedIn).toBe(ADA.username)
    expect(rowOf(CY.username)).toContainElement(shown)
    expect(screen.queryByRole('form', { name: `New password for ${CY.username}` })).not.toBeInTheDocument()
    expect(shown).toHaveTextContent(window.location.origin)
    expect(shown).toHaveTextContent(CY.username)
    expect(shown).toHaveTextContent(generated)
    expect(shown).toHaveTextContent('only time the password is shown')
    // On a phone the panel is below the fold: it scrolls itself into view and takes the focus.
    expect(scroll).toHaveBeenCalled()
    expect(scroll.mock.contexts.at(-1)).toBe(shown)
    const copy = within(shown).getByRole('button', { name: 'Copy both, with the site address' })
    expect(copy).toHaveFocus()

    await app.user.click(copy)
    expect(writeText).toHaveBeenCalledWith(
      `Address: ${window.location.origin}\nUsername: ${CY.username}\nPassword: ${generated}`,
    )
    const trigger = await newPasswordButton()
    await app.user.click(within(shown).getByRole('button', { name: 'Done' }))
    expect(screen.queryByRole('region', { name: /^The new password/ })).not.toBeInTheDocument()
    await waitFor(() => expect(trigger).toHaveFocus())
    // Kept nowhere once it is dismissed.
    expect(document.body).not.toHaveTextContent(generated)
    const stored = JSON.stringify({ ...window.localStorage }) + JSON.stringify({ ...window.sessionStorage })
    expect(stored).not.toContain(generated)
    expect(app.router.state.location.href).not.toContain(generated)
    expect(JSON.stringify(app.store.getState())).not.toContain(generated)
  })

  it('sends exactly what was typed, spaces and all', async () => {
    const app = await open()
    await app.user.click(await newPasswordButton('Di_4'))
    await app.user.type(within(resetForm('Di_4')).getByLabelText('New password'), '  spaced out pass  ')
    await app.user.keyboard('{Enter}')
    await screen.findByRole('region', { name: 'The new password for Di_4' })
    expect(app.accounts.resets).toEqual([{ id: idOf('Di_4'), password: '  spaced out pass  ' }])
  })

  it('disables the form and says what it is doing while the answer is awaited, and sends nothing twice', async () => {
    let answer!: (response: Response) => void
    const app = await open({
      override: { resetPassword: () => new Promise<Response>((resolve) => (answer = resolve)) },
    })
    await app.user.click(await newPasswordButton())
    await app.user.type(passwordField(), FRESH)
    await app.user.click(within(resetForm()).getByRole('button', { name: 'Set password' }))
    const busy = await within(resetForm()).findByRole('button', { name: 'Setting the password…' })
    expect(busy).toHaveAttribute('aria-disabled', 'true')
    expect(resetForm()).toHaveAttribute('aria-busy', 'true')
    expect(passwordField()).toHaveAttribute('readonly')
    await app.user.keyboard('{Enter}')
    answer(new Response(null, { status: 204 }))
    await screen.findByRole('region', { name: `The new password for ${CY.username}` })
    expect(sentTo(app, `/api/admin/users/${idOf(CY.username)}/password`, 'POST')).toHaveLength(1)
  })

  it.each([
    [
      'closes with Cancel',
      async (app: App) => app.user.click(screen.getByRole('button', { name: 'Cancel' })),
    ],
    ['closes with Escape', async (app: App) => app.user.keyboard('{Escape}')],
  ])('%s, sends nothing and gives the focus back to the button', async (_how, close) => {
    const app = await open()
    const trigger = await newPasswordButton()
    await app.user.click(trigger)
    await app.user.type(passwordField(), FRESH)
    await close(app)
    expect(screen.queryByRole('form', { name: `New password for ${CY.username}` })).not.toBeInTheDocument()
    await waitFor(() => expect(trigger).toHaveFocus())
    expect(sentTo(app, `/api/admin/users/${idOf(CY.username)}/password`, 'POST')).toHaveLength(0)
    // Opened again, the field is empty.
    await app.user.click(trigger)
    expect(passwordField()).toHaveValue('')
  })

  it('checks the rules before sending anything and takes the complaint back once it is fixed', async () => {
    const app = await open()
    await app.user.click(await newPasswordButton())
    await app.user.type(passwordField(), 'short')
    await app.user.click(within(resetForm()).getByRole('button', { name: 'Set password' }))
    await waitFor(() => expect(passwordField()).toHaveAttribute('aria-invalid', 'true'))
    expect(passwordField()).toHaveAccessibleDescription(/at least 10 characters/)
    expect(passwordField()).toHaveFocus()
    expect(app.accounts.resets).toHaveLength(0)
    await app.user.type(passwordField(), ' and longer')
    expect(passwordField()).toHaveAttribute('aria-invalid', 'false')
  })

  it('puts a password the server refuses on the field, in plain words, and keeps what was typed', async () => {
    const app = await open({ override: { resetPassword: () => authError(400, 'invalid_password') } })
    await app.user.click(await newPasswordButton())
    await app.user.type(passwordField(), FRESH)
    await app.user.click(within(resetForm()).getByRole('button', { name: 'Set password' }))
    await waitFor(() => expect(passwordField()).toHaveAttribute('aria-invalid', 'true'))
    expect(passwordField()).toHaveAccessibleDescription(/does not accept that password/)
    expect(passwordField()).toHaveValue(FRESH)
    expect(passwordField()).toHaveFocus()
  })

  it.each([
    ['too_many_attempts', 429, /changed many accounts in a short time.*about 15 minutes/i],
    ['unavailable', 503, /cannot check accounts right now/i],
    ['internal', 500, /ran into a problem/i],
    ['admin_account', 409, /Change its password under Account settings/],
  ] as const)('shows %s as an alert about the form with focus on it', async (code, status, words) => {
    const app = await open({
      override: {
        resetPassword: () =>
          authError(
            status,
            code,
            'refused',
            code === 'too_many_attempts' ? { 'retry-after': '900' } : undefined,
          ),
      },
    })
    await app.user.click(await newPasswordButton())
    await app.user.type(passwordField(), FRESH)
    await app.user.click(within(resetForm()).getByRole('button', { name: 'Set password' }))
    const alert = await within(resetForm()).findByRole('alert')
    expect(within(alert).getByText('Could not set the password')).toBeInTheDocument()
    expect(alert).toHaveTextContent(words)
    await waitFor(() => expect(alert).toHaveFocus())
    expect(passwordField()).toHaveValue(FRESH)
  })

  it('says the account is gone and loads the list again when the server knows no such account', async () => {
    const app = await open({ override: { resetPassword: () => authError(404, 'unknown_account') } })
    const before = listings(app)
    await app.user.click(await newPasswordButton())
    await app.user.type(passwordField(), FRESH)
    await app.user.click(within(resetForm()).getByRole('button', { name: 'Set password' }))
    expect(await within(resetForm()).findByRole('alert')).toHaveTextContent(/already gone/)
    await waitFor(() => expect(listings(app)).toBe(before + 1))
  })

  it('turns to the sign-in page when the session has ended', async () => {
    const app = await open({ override: { resetPassword: () => authError(401, 'unauthenticated') } })
    await app.user.click(await newPasswordButton())
    await app.user.type(passwordField(), FRESH)
    await app.user.click(within(resetForm()).getByRole('button', { name: 'Set password' }))
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/login'))
  })
})

describe('removing an account', () => {
  const dialog = () => screen.findByRole('alertdialog')

  it('asks first, says plainly what is deleted, starts on the safe button and sends nothing yet', async () => {
    const app = await open()
    await app.user.click(await removeButton())
    const box = within(await dialog())
    expect(box.getByRole('heading', { name: `Remove ${CY.username}?` })).toBeInTheDocument()
    const words = box.getByText(/deletes the account of Cy_3/)
    expect(words).toHaveTextContent(/all of their decks/)
    expect(words).toHaveTextContent(/lobbies they host are closed/)
    expect(words).toHaveTextContent(/leave the games they are in/)
    expect(words).toHaveTextContent(/cannot be undone/)
    await waitFor(() => expect(box.getByRole('button', { name: 'Keep account' })).toHaveFocus())
    expect(sentTo(app, `/api/admin/users/${idOf(CY.username)}`, 'DELETE')).toHaveLength(0)
    expect(app.accounts.users.has(CY.username)).toBe(true)
  })

  it.each([
    [
      'Keep account',
      async (app: App) => app.user.click(screen.getByRole('button', { name: 'Keep account' })),
    ],
    ['Escape', async (app: App) => app.user.keyboard('{Escape}')],
  ])('keeps the account on %s and gives the focus back to the button', async (_how, close) => {
    const app = await open()
    const trigger = await removeButton()
    await app.user.click(trigger)
    await dialog()
    await close(app)
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    await waitFor(() => expect(trigger).toHaveFocus())
    expect(sentTo(app, `/api/admin/users/${idOf(CY.username)}`, 'DELETE')).toHaveLength(0)
    expect(rowOf(CY.username)).toBeInTheDocument()
  })

  it('removes the account, drops its row, says so, and puts the focus on the list', async () => {
    const app = await open()
    await app.user.click(await removeButton())
    const box = within(await dialog())
    await app.user.click(box.getByRole('button', { name: 'Remove account' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    expect(sentTo(app, `/api/admin/users/${idOf(CY.username)}`, 'DELETE')).toHaveLength(1)
    expect(app.accounts.removed).toEqual([idOf(CY.username)])
    expect(app.accounts.users.has(CY.username)).toBe(false)
    const list = screen.getByRole('region', { name: 'Accounts' })
    expect(await within(list).findByText('Cy_3 was removed.')).toBeInTheDocument()
    expect(within(list).queryByText(CY.username, { selector: 'span' })).not.toBeInTheDocument()
    expect(within(list).getByText('2 accounts, oldest first.')).toBeInTheDocument()
    expect(within(list).getByRole('heading', { name: 'Accounts' })).toHaveFocus()
    expect(app.accounts.signedIn).toBe(ADA.username)
  })

  it('keeps the question up with the reason when the server cannot remove it', async () => {
    const app = await open({ override: { removeAccount: () => authError(503, 'unavailable') } })
    await app.user.click(await removeButton())
    const box = within(await dialog())
    await app.user.click(box.getByRole('button', { name: 'Remove account' }))
    expect(await box.findByText(/cannot check accounts right now/i)).toBeInTheDocument()
    expect(app.accounts.users.has(CY.username)).toBe(true)
    await app.user.click(box.getByRole('button', { name: 'Keep account' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    expect(rowOf(CY.username)).toBeInTheDocument()
  })

  it('says the admin account cannot be deleted if the server says so', async () => {
    const app = await open({ override: { removeAccount: () => authError(409, 'admin_account') } })
    await app.user.click(await removeButton())
    const box = within(await dialog())
    await app.user.click(box.getByRole('button', { name: 'Remove account' }))
    expect(
      await box.findByText('This account makes accounts for your friends, so it cannot be deleted.'),
    ).toBeInTheDocument()
  })

  it('counts an account that is already gone as removed, and loads the list again', async () => {
    const app = await open({ override: { removeAccount: () => authError(404, 'unknown_account') } })
    const before = listings(app)
    await app.user.click(await removeButton())
    const box = within(await dialog())
    await app.user.click(box.getByRole('button', { name: 'Remove account' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    await waitFor(() => expect(listings(app)).toBeGreaterThan(before))
  })

  it('learns from the server that the account is no longer the admin', async () => {
    const app = await open({ override: { removeAccount: () => authError(403, 'forbidden') } })
    await app.user.click(await removeButton())
    await app.user.click(within(await dialog()).getByRole('button', { name: 'Remove account' }))
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Only the admin can make accounts.' }),
    ).toBeInTheDocument()
  })
})
