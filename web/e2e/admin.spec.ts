// Goal: the admin's journey on a real server and database: the admin signs in and finds Accounts, makes an account with a generated password, the friend signs in with what the panel showed (and with nothing else), the friend does not see Accounts and is told so at its address, and a server that closed registration shows the closed sign-up page.
import { execFileSync } from 'node:child_process'
import { expect, test } from '@playwright/test'
import type { Browser, Page } from '@playwright/test'
import { ACCOUNTS_DATABASE_URL, ACCOUNTS_WEB_URL } from './ports'

// Needs Postgres like accounts.spec.ts: skipped without E2E_DATABASE_URL.
test.skip(
  ACCOUNTS_DATABASE_URL === undefined,
  'set E2E_DATABASE_URL to an empty scratch database to run this',
)

const PASSWORD = 'correct horse battery'

const unique = (prefix: string): string => `${prefix}_${Math.random().toString(36).slice(2, 8)}`

/**
 * Marks an account as the admin in the scratch database. The server sets the flag only at start, from
 * `MTG_ADMIN_USERNAME`, and the account has to exist by then; a spec that registers its own admin sets the flag the
 * way the server does. Uses `psql`, or the one in the local `magic-db-1` container when the host has none.
 */
function makeAdmin(username: string): void {
  const url = new URL(ACCOUNTS_DATABASE_URL ?? '')
  const sql = `update users set is_admin = true where username = '${username}'`
  try {
    execFileSync('psql', [url.href, '-c', sql], { stdio: 'pipe' })
  } catch {
    const database = url.pathname.slice(1)
    execFileSync('docker', ['exec', 'magic-db-1', 'psql', '-U', url.username, '-d', database, '-c', sql], {
      stdio: 'pipe',
    })
  }
}

async function visitor(browser: Browser): Promise<{ page: Page; errors: string[] }> {
  const context = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL })
  const page = await context.newPage()
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  return { page, errors }
}

const account = (page: Page) => page.getByRole('navigation', { name: 'Account' })

test('the admin makes an account and the friend signs in with it', async ({ browser }) => {
  const admin = await visitor(browser)
  const friend = await visitor(browser)
  const owner = unique('owner')

  // The owner registers, is named the admin, and signs in again to see it.
  await admin.page.goto('/register')
  await admin.page.getByLabel('Username').fill(owner)
  await admin.page.getByLabel('Password').fill(PASSWORD)
  await admin.page.getByRole('button', { name: 'Create account' }).click()
  await expect(admin.page.getByRole('heading', { level: 1, name: 'Play' })).toBeVisible()
  makeAdmin(owner)
  await admin.page.reload()
  await account(admin.page).getByRole('button', { name: owner }).click()
  await admin.page.getByRole('menuitem', { name: 'Accounts' }).click()
  await expect(admin.page.getByRole('heading', { name: 'Accounts', exact: true })).toBeVisible()

  // A generated password is shown once, with the address and the username.
  const friendName = unique('friend')
  await admin.page.getByLabel('Username').fill(friendName)
  await admin.page.getByRole('button', { name: 'Make a password' }).click()
  await admin.page.getByRole('button', { name: 'Make account' }).click()
  const panel = admin.page.getByRole('region', { name: 'The new account' })
  await expect(panel).toBeVisible()
  await expect(panel.getByText(friendName, { exact: true })).toBeVisible()
  const password = (await panel.locator('dt:text-is("Password") + dd').textContent()) ?? ''
  expect(password).toHaveLength(16)
  await expect(admin.page.getByRole('list').getByText(friendName, { exact: true })).toBeVisible()
  await panel.getByRole('button', { name: 'Done' }).click()
  await expect(panel).toBeHidden()

  // The friend signs in with those words; the admin's session was not touched.
  await friend.page.goto('/login')
  await friend.page.getByLabel('Username').fill(friendName)
  await friend.page.getByLabel('Password').fill(password)
  await friend.page.getByRole('button', { name: 'Sign in' }).click()
  await expect(friend.page.getByRole('heading', { level: 1, name: 'Play' })).toBeVisible()
  await account(friend.page).getByRole('button', { name: friendName }).click()
  await expect(friend.page.getByRole('menuitem', { name: 'Account settings' })).toBeVisible()
  await expect(friend.page.getByRole('menuitem', { name: 'Accounts' })).toHaveCount(0)
  await friend.page.keyboard.press('Escape')

  // At the address the friend is told that only the admin can.
  await friend.page.goto('/accounts')
  await expect(friend.page.getByText('Only the admin can make accounts.')).toBeVisible()

  expect(admin.errors).toEqual([])
  expect(friend.errors).toEqual([])
})

test('a server with registration closed shows the closed sign-up page', async ({ browser }) => {
  const guest = await visitor(browser)
  // This server keeps registration open for the other specs; the options answer is what a closed one gives.
  await guest.page.route('**/api/auth/options', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ registration_open: false }),
    }),
  )
  await guest.page.goto('/register')
  await expect(guest.page.getByRole('heading', { name: 'New accounts are made by the admin' })).toBeVisible()
  await expect(guest.page.getByLabel('Username')).toHaveCount(0)
  await guest.page.getByRole('link', { name: 'Sign in' }).last().click()
  await expect(guest.page).toHaveURL(/\/login/)
  expect(guest.errors).toEqual([])
})
