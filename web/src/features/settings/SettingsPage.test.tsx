// Goal: the settings page sends exactly the typed passwords once, checks the obvious rules before sending, puts every refusal in plain words on the right field with focus on the first problem, empties the passwords it holds, keeps a signed-in session signed in after a change, asks for the password in a dialog before deleting the account and goes home with a notice when it is gone, and sends a visitor without a session to sign in and back.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { flashStore } from '@/lib/flash'
import { authError } from '@/test-utils/accountsServer'
import type { AccountsScript } from '@/test-utils/accountsServer'
import { renderApp } from '@/test-utils/appHarness'
import type { App } from '@/test-utils/appHarness'

afterEach(() => {
  vi.unstubAllGlobals()
  flashStore.setState({ message: null })
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const NEW = 'a brand new passphrase'
const users = { [ADA.username]: ADA.password }

const form = () => screen.getByRole('form', { name: 'Change password' })
const current = () => screen.getByLabelText('Current password')
const next = () => screen.getByLabelText('New password')
const again = () => screen.getByLabelText('New password again')
const submit = () => within(form()).getByRole('button', { name: 'Change password' })
const dialog = () => screen.findByRole('alertdialog')
const sentTo = (app: App, path: string) => app.accounts.calls.filter((call) => call.url === path)
const bodyOf = (call: { init?: RequestInit } | undefined): unknown => JSON.parse(String(call?.init?.body))

async function open(accounts: AccountsScript = {}, url = '/settings') {
  const app = await renderApp(url, { accounts: { users, signedIn: ADA.username, ...accounts } })
  await waitFor(() => expect(app.store.getState().status).not.toBe('loading'))
  return app
}

async function fill(app: App, values: { current?: string; next?: string; again?: string }) {
  if (values.current !== undefined) await app.user.type(current(), values.current)
  if (values.next !== undefined) await app.user.type(next(), values.next)
  if (values.again !== undefined) await app.user.type(again(), values.again)
}

describe('the page', () => {
  it('is a titled page with the change-password form and a way to delete the account', async () => {
    await open()
    const heading = await screen.findByRole('heading', { level: 1, name: 'Account settings' })
    expect(document.title).toBe('Account settings · Magic')
    expect(heading.parentElement).toHaveTextContent(`Signed in as ${ADA.username}.`)
    expect(form()).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 2, name: 'Delete account' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^Delete account/ })).toBeEnabled()
  })

  it('is reached from the account menu in the header', async () => {
    const app = await open({}, '/')
    await app.user.click(screen.getByRole('button', { name: /signed in as/i }))
    await app.user.click(await screen.findByRole('menuitem', { name: 'Account settings' }))
    expect(await screen.findByRole('heading', { level: 1, name: 'Account settings' })).toBeInTheDocument()
    expect(app.router.state.location.pathname).toBe('/settings')
  })

  it('tells password managers which field is which and keeps the passwords out of the address', async () => {
    await open()
    expect(await screen.findByRole('form', { name: 'Change password' })).toHaveAttribute('method', 'post')
    expect(current()).toHaveAttribute('autocomplete', 'current-password')
    expect(next()).toHaveAttribute('autocomplete', 'new-password')
    expect(again()).toHaveAttribute('autocomplete', 'new-password')
    for (const field of [current(), next(), again()]) expect(field).toHaveAttribute('type', 'password')
    expect(next()).toHaveAccessibleDescription(/10 to 128 characters/)
  })

  it('sends a visitor without a session to sign in, and back here afterwards', async () => {
    const app = await renderApp('/settings', { accounts: { users } })
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/login'))
    expect(app.router.state.location.search).toEqual({ next: '/settings' })
  })

  it('says a server without accounts has none to change', async () => {
    await renderApp('/settings', { accounts: { unavailable: true } })
    expect(await screen.findByRole('heading', { name: 'This server has no accounts' })).toBeInTheDocument()
    expect(screen.queryByRole('form', { name: 'Change password' })).not.toBeInTheDocument()
  })
})

describe('changing the password', () => {
  it('sends both passwords once, empties the fields, announces it and stays signed in', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Change password' })
    await fill(app, { current: ADA.password, next: NEW, again: NEW })
    await app.user.click(submit())

    expect(await screen.findByText(/password changed/i)).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent(/other sessions are signed out/i)
    const sent = sentTo(app, '/api/auth/password')
    expect(sent).toHaveLength(1)
    expect(bodyOf(sent[0])).toEqual({ current_password: ADA.password, new_password: NEW })
    for (const field of [current(), next(), again()]) expect(field).toHaveValue('')
    expect(app.accounts.users.get(ADA.username)).toBe(NEW)
    expect(app.store.getState()).toMatchObject({ status: 'signed-in', user: { username: ADA.username } })
    expect(app.router.state.location.pathname).toBe('/settings')
  })

  it('does not send a form that breaks an obvious rule, and focuses the first wrong field', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Change password' })

    await app.user.click(submit())
    expect(await screen.findByText('Enter your current password.')).toBeInTheDocument()
    expect(current()).toHaveAttribute('aria-invalid', 'true')
    expect(current()).toHaveFocus()

    await fill(app, { current: ADA.password, next: 'short', again: 'short' })
    await app.user.click(submit())
    expect(await screen.findByText(/needs at least 10 characters \(this one has 5\)/)).toBeInTheDocument()
    expect(next()).toHaveFocus()

    await app.user.clear(next())
    await app.user.clear(again())
    await fill(app, { next: NEW, again: `${NEW} typo` })
    await app.user.click(submit())
    expect(await screen.findByText('The two new passwords are not the same.')).toBeInTheDocument()
    expect(again()).toHaveFocus()
    expect(sentTo(app, '/api/auth/password')).toHaveLength(0)
  })

  it('puts a wrong current password on its field, empties it and keeps the new ones', async () => {
    const app = await open()
    await screen.findByRole('form', { name: 'Change password' })
    await fill(app, { current: 'not my password', next: NEW, again: NEW })
    await app.user.click(submit())

    expect(await screen.findByText(/not your current password/i)).toBeInTheDocument()
    expect(current()).toHaveAttribute('aria-invalid', 'true')
    expect(current()).toHaveAccessibleDescription(/not your current password/i)
    expect(current()).toHaveValue('')
    expect(current()).toHaveFocus()
    expect(next()).toHaveValue(NEW)
    expect(screen.queryByText(/password changed/i)).not.toBeInTheDocument()
    expect(app.accounts.users.get(ADA.username)).toBe(ADA.password)
  })

  it('names the rules when the server refuses the new password, on the new password field', async () => {
    const app = await open({
      override: { password: () => authError(400, 'invalid_password', 'a password has 10 to 128 characters') },
    })
    await screen.findByRole('form', { name: 'Change password' })
    await fill(app, { current: ADA.password, next: NEW, again: NEW })
    await app.user.click(submit())
    expect(await screen.findByText(/server does not accept that password/i)).toBeInTheDocument()
    expect(next()).toHaveAttribute('aria-invalid', 'true')
    expect(next()).toHaveFocus()
  })

  it('says to wait when the sign-in tries are used up, in an alert about the whole form', async () => {
    const app = await open({ throttleAfter: 1 })
    await screen.findByRole('form', { name: 'Change password' })
    await fill(app, { current: 'wrong one one one', next: NEW, again: NEW })
    await app.user.click(submit())
    await screen.findByText(/not your current password/i)

    await fill(app, { current: ADA.password })
    await app.user.click(submit())
    const alert = await screen.findByText(/too many failed attempts/i)
    expect(alert).toHaveTextContent(/wait about 15 minutes/i)
    expect(alert.closest('[role="alert"]')).toHaveFocus()
  })

  it('reports a server that cannot be reached and keeps what was typed', async () => {
    const app = await open({ override: { password: () => Promise.reject(new TypeError('offline')) } })
    await screen.findByRole('form', { name: 'Change password' })
    await fill(app, { current: ADA.password, next: NEW, again: NEW })
    await app.user.click(submit())
    expect(await screen.findByText(/could not reach the server/i)).toBeInTheDocument()
    expect(next()).toHaveValue(NEW)
  })

  it('goes to sign in when the session ended meanwhile', async () => {
    const app = await open({ override: { password: () => authError(401, 'unauthenticated') } })
    await screen.findByRole('form', { name: 'Change password' })
    await fill(app, { current: ADA.password, next: NEW, again: NEW })
    await app.user.click(submit())
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/login'))
    expect(app.store.getState().status).toBe('signed-out')
  })
})

describe('deleting the account', () => {
  const deleteButton = () => screen.getByRole('button', { name: /^Delete account/ })

  async function ask(app: App) {
    await screen.findByRole('form', { name: 'Change password' })
    await app.user.click(deleteButton())
    return within(await dialog())
  }

  it('asks first, in a dialog that names what is at stake, and sends nothing until it is confirmed', async () => {
    const app = await open()
    const box = await ask(app)
    expect(box.getByRole('heading', { name: 'Delete your account?' })).toBeInTheDocument()
    expect(box.getByLabelText('Password')).toHaveAttribute('autocomplete', 'current-password')
    expect(sentTo(app, '/api/auth/account')).toHaveLength(0)

    await app.user.click(box.getByRole('button', { name: 'Keep account' }))
    await waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument())
    expect(sentTo(app, '/api/auth/account')).toHaveLength(0)
    expect(app.store.getState().status).toBe('signed-in')
  })

  it('does not offer the admin account to be deleted, and says why in plain words', async () => {
    const app = await open({ admin: ADA.username })
    await screen.findByRole('form', { name: 'Change password' })
    const section = screen.getByRole('region', { name: 'Delete account' })
    expect(section).toHaveTextContent(
      'This account makes accounts for your friends, so it cannot be deleted.',
    )
    expect(section).not.toHaveTextContent(/your decks are deleted/i)
    expect(deleteButton()).toBeDisabled()
    expect(deleteButton()).toHaveAccessibleDescription(/cannot be deleted/)
    await app.user.click(deleteButton())
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()
    expect(sentTo(app, '/api/auth/account')).toHaveLength(0)
    expect(app.store.getState().status).toBe('signed-in')
  })

  it('still offers every other account the deletion', async () => {
    await open({ admin: 'someone_else' })
    await screen.findByRole('form', { name: 'Change password' })
    const section = screen.getByRole('region', { name: 'Delete account' })
    expect(section).toHaveTextContent('This cannot be undone.')
    expect(section).not.toHaveTextContent(/makes accounts for your friends/)
    expect(deleteButton()).toBeEnabled()
  })

  it('says the same words if the server refuses the admin account after all', async () => {
    const app = await open({ override: { account: () => authError(409, 'admin_account') } })
    const box = await ask(app)
    await app.user.type(box.getByLabelText('Password'), ADA.password)
    await app.user.click(box.getByRole('button', { name: 'Delete account' }))
    expect(
      await box.findByText('This account makes accounts for your friends, so it cannot be deleted.'),
    ).toBeInTheDocument()
    expect(app.store.getState().status).toBe('signed-in')
  })

  it('wants a password before it sends anything', async () => {
    const app = await open()
    const box = await ask(app)
    await app.user.click(box.getByRole('button', { name: 'Delete account' }))
    expect(await box.findByText('Enter your password to confirm.')).toBeInTheDocument()
    expect(box.getByLabelText('Password')).toHaveFocus()
    expect(sentTo(app, '/api/auth/account')).toHaveLength(0)
  })

  it('keeps the question up when the password is wrong, with the reason, and an empty field', async () => {
    const app = await open()
    const box = await ask(app)
    await app.user.type(box.getByLabelText('Password'), 'not my password')
    await app.user.click(box.getByRole('button', { name: 'Delete account' }))

    expect(await box.findByText(/not your current password/i)).toBeInTheDocument()
    expect(screen.getByRole('alertdialog')).toBeInTheDocument()
    expect(box.getByLabelText('Password')).toHaveValue('')
    expect(box.getByLabelText('Password')).toHaveFocus()
    expect(app.store.getState().status).toBe('signed-in')
    expect(app.accounts.users.has(ADA.username)).toBe(true)
  })

  it('deletes the account with the typed password, goes home signed out and says so', async () => {
    const app = await open()
    const box = await ask(app)
    await app.user.type(box.getByLabelText('Password'), ADA.password)
    await app.user.click(box.getByRole('button', { name: 'Delete account' }))

    await waitFor(() => expect(app.router.state.location.pathname).toBe('/'))
    expect(await screen.findByText('Your account was deleted.')).toBeInTheDocument()
    const sent = sentTo(app, '/api/auth/account')
    expect(sent).toHaveLength(1)
    expect(sent[0]?.init?.method).toBe('DELETE')
    expect(bodyOf(sent[0])).toEqual({ password: ADA.password })
    expect(app.accounts.users.has(ADA.username)).toBe(false)
    expect(app.store.getState()).toMatchObject({ status: 'signed-out', user: null })
    // The page did not stop at the sign-in form on its way.
    expect(app.router.history.location.pathname).toBe('/')
    expect(app.router.history.length).toBeLessThanOrEqual(1)
  })

  it('says to wait when the tries are used up, and deletes nothing', async () => {
    const app = await open({ throttleAfter: 1 })
    const box = await ask(app)
    await app.user.type(box.getByLabelText('Password'), 'wrong one one one')
    await app.user.click(box.getByRole('button', { name: 'Delete account' }))
    await box.findByText(/not your current password/i)

    await app.user.type(box.getByLabelText('Password'), ADA.password)
    await app.user.click(box.getByRole('button', { name: 'Delete account' }))
    expect(await box.findByText(/too many failed attempts.*about 15 minutes/i)).toBeInTheDocument()
    expect(app.accounts.users.has(ADA.username)).toBe(true)
  })

  it('stays on the page, signed in, when the server could not delete', async () => {
    const app = await open({ override: { account: () => authError(503, 'unavailable') } })
    const box = await ask(app)
    await app.user.type(box.getByLabelText('Password'), ADA.password)
    await app.user.click(box.getByRole('button', { name: 'Delete account' }))
    expect(await box.findByText(/cannot check accounts right now/i)).toBeInTheDocument()
    expect(app.store.getState().status).toBe('signed-in')
    expect(app.router.state.location.pathname).toBe('/settings')
  })
})
