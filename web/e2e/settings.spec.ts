// Goal: the account settings work end to end in a browser on a real server and database: the page needs a session and brings the visitor back after sign-in, a changed password signs the other browser out and replaces the old one, and a deleted account goes home signed out, loses its games and lets its username be registered again.
import { expect, test } from '@playwright/test'
import type { Browser, BrowserContext, Page } from '@playwright/test'
import { ACCOUNTS_DATABASE_URL, ACCOUNTS_WEB_URL } from './ports'

// Accounts need Postgres, so this spec runs only against its own server on an empty scratch database: set
// E2E_DATABASE_URL (docs/web.md says how). Without it `pnpm e2e` has no database and this file is skipped.
test.skip(
  ACCOUNTS_DATABASE_URL === undefined,
  'set E2E_DATABASE_URL to an empty scratch database to run this',
)

const PASSWORD = 'correct horse battery'
const NEW_PASSWORD = 'another long passphrase'

/** A username no earlier run used, so a scratch database that is kept between runs does not say "taken". */
const unique = (prefix: string): string => `${prefix}_${Math.random().toString(36).slice(2, 8)}`

/** One browser, with its own cookies, and the page's uncaught exceptions collected: the UI should never throw. */
interface Visitor {
  context: BrowserContext
  page: Page
  errors: string[]
}

async function visitor(browser: Browser): Promise<Visitor> {
  const context = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL })
  const page = await context.newPage()
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  return { context, page, errors }
}

const account = (page: Page) => page.getByRole('navigation', { name: 'Account' })
const createForm = (page: Page) => page.getByRole('form', { name: 'Create a game' })

async function register(page: Page, username: string, password = PASSWORD): Promise<void> {
  await page.goto('/register')
  await page.getByLabel('Username').fill(username)
  await page.getByLabel('Password').fill(password)
  await page.getByRole('button', { name: 'Create account' }).click()
  await expect(account(page).getByText(username)).toBeVisible()
}

async function signIn(page: Page, username: string, password: string): Promise<void> {
  await page.getByLabel('Username').fill(username)
  await page.getByLabel('Password').fill(password)
  await page.getByRole('button', { name: 'Sign in', exact: true }).click()
}

/** Opens the page from the account menu, as a person would. */
async function openSettings(page: Page): Promise<void> {
  await account(page)
    .getByRole('button', { name: /signed in as/i })
    .click()
  await page.getByRole('menuitem', { name: 'Account settings' }).click()
  await expect(page.getByRole('heading', { level: 1, name: 'Account settings' })).toBeVisible()
}

async function changePassword(page: Page, current: string, next: string): Promise<void> {
  await page.getByLabel('Current password').fill(current)
  await page.getByLabel('New password', { exact: true }).fill(next)
  await page.getByLabel('New password again').fill(next)
  await page.getByRole('button', { name: 'Change password' }).click()
}

test('a changed password signs the other browser out, replaces the old password and keeps this browser in', async ({
  browser,
}) => {
  const ada = unique('ada')
  const laptop = await visitor(browser)
  const phone = await visitor(browser)
  const stranger = await visitor(browser)
  try {
    await register(laptop.page, ada)
    await phone.page.goto('/login')
    await signIn(phone.page, ada, PASSWORD)
    await expect(account(phone.page).getByText(ada)).toBeVisible()

    // Without a session the page asks for one and comes back to the settings afterwards.
    await stranger.page.goto('/settings')
    await expect(stranger.page).toHaveURL(/\/login\?next=%2Fsettings$/)
    await signIn(stranger.page, ada, PASSWORD)
    await expect(stranger.page.getByRole('heading', { level: 1, name: 'Account settings' })).toBeVisible()
    await stranger.context.close()

    await openSettings(laptop.page)
    // A wrong current password is said on its field and changes nothing.
    await changePassword(laptop.page, 'not my password', NEW_PASSWORD)
    await expect(laptop.page.getByText('That is not your current password.')).toBeVisible()
    await expect(laptop.page.getByLabel('Current password')).toHaveValue('')

    await changePassword(laptop.page, PASSWORD, NEW_PASSWORD)
    await expect(laptop.page.getByText('Password changed. Your other sessions are signed out.')).toBeVisible()
    await expect(account(laptop.page).getByText(ada)).toBeVisible()

    // The other browser's session ended with the old password.
    await phone.page.goto('/settings')
    await expect(phone.page).toHaveURL(/\/login\?next=%2Fsettings$/)
    await expect(account(phone.page).getByRole('link', { name: 'Sign in' })).toBeVisible()

    // The old password no longer signs in; the new one does.
    await signIn(phone.page, ada, PASSWORD)
    await expect(phone.page.getByText('The username or password is wrong.')).toBeVisible()
    await signIn(phone.page, ada, NEW_PASSWORD)
    await expect(phone.page.getByRole('heading', { level: 1, name: 'Account settings' })).toBeVisible()

    // This browser kept its session through the change.
    await laptop.page.reload()
    await expect(laptop.page.getByRole('heading', { level: 1, name: 'Account settings' })).toBeVisible()
    expect([...laptop.errors, ...phone.errors]).toEqual([])
  } finally {
    await laptop.context.close()
    await phone.context.close()
  }
})

test('a deleted account goes home signed out, loses its games and its username can be registered again', async ({
  browser,
}) => {
  const ada = unique('ada')
  const { context, page, errors } = await visitor(browser)
  try {
    await register(page, ada)
    await page.goto('/new')
    await expect(createForm(page)).toBeVisible()
    await page.getByRole('button', { name: 'Create game' }).click()
    await expect(page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    await page.goto('/games')
    await expect(page.locator('main [data-game]')).toHaveCount(1)

    await openSettings(page)
    await page.getByRole('button', { name: /^Delete account/ }).click()
    const dialog = page.getByRole('alertdialog')
    await expect(dialog.getByRole('heading', { name: 'Delete your account?' })).toBeVisible()

    // A wrong password keeps the question up and deletes nothing.
    await dialog.getByLabel('Password').fill('not my password')
    await dialog.getByRole('button', { name: 'Delete account' }).click()
    await expect(dialog.getByText('That is not your current password.')).toBeVisible()
    await expect(dialog.getByLabel('Password')).toHaveValue('')

    await dialog.getByLabel('Password').fill(PASSWORD)
    await dialog.getByRole('button', { name: 'Delete account' }).click()
    await expect(page).toHaveURL(/\/$/)
    await expect(page.locator('[data-slot="flash-notice"]')).toContainText('Your account was deleted.')
    await expect(account(page).getByRole('link', { name: 'Sign in' })).toBeVisible()

    // The account is gone: it cannot sign in, and the page for signed-in people asks for a session.
    await page.goto('/login')
    await signIn(page, ada, PASSWORD)
    await expect(page.getByText('The username or password is wrong.')).toBeVisible()
    await page.goto('/settings')
    await expect(page).toHaveURL(/\/login\?next=%2Fsettings$/)

    // The username is free again, and the new account holds nothing of the old one.
    await register(page, ada, NEW_PASSWORD)
    await page.goto('/games')
    await expect(page.getByRole('heading', { name: 'You have no games yet' })).toBeVisible()
    await expect(page.locator('main [data-game]')).toHaveCount(0)
    expect(errors).toEqual([])
  } finally {
    await context.close()
  }
})
