// Goal: on a real server and database a signed-in player who fills a public lobby with bots and closes the app without pressing Ready finds it again on Play: the open games list marks it "Your seat" with Open in place of a disabled Join, a "Back to your lobby" link leads there too, and both reopen the lobby on the same seat, while another account still sees the lobby full with Join disabled.
import { expect, test } from '@playwright/test'
import type { Browser, BrowserContext, Page } from '@playwright/test'
import { ACCOUNTS_DATABASE_URL, ACCOUNTS_WEB_URL } from './ports'

test.skip(
  ACCOUNTS_DATABASE_URL === undefined,
  'set E2E_DATABASE_URL to an empty scratch database to run this',
)

const PASSWORD = 'correct horse battery'

const unique = (prefix: string): string => `${prefix}_${Math.random().toString(36).slice(2, 8)}`

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

async function register(page: Page, username: string): Promise<void> {
  await page.goto('/register')
  await page.getByLabel('Username').fill(username)
  await page.getByLabel('Password').fill(PASSWORD)
  await page.getByRole('button', { name: 'Create account' }).click()
  await expect(page.getByRole('heading', { level: 1, name: 'Play' })).toBeVisible()
}

test('a player who filled a lobby with bots and closed the app reopens it from Play', async ({ browser }) => {
  test.setTimeout(90_000)
  const [ada, bo] = [await visitor(browser), await visitor(browser)]
  try {
    const adaName = unique('ada')
    await register(ada.page, adaName)
    await ada.page.goto('/new')
    await ada.page.getByRole('radio', { name: /^Normal/ }).check()
    await ada.page.getByLabel('Bots').selectOption('1')
    await ada.page.getByRole('button', { name: 'Create game' }).click()
    await expect(ada.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    const lobby = new URL(ada.page.url()).pathname

    // The app closes without Ready; it opens again on Play.
    await ada.page.close()
    const page = await ada.context.newPage()
    page.on('pageerror', (error) => ada.errors.push(error.message))
    await page.goto('/')
    const card = page.getByRole('article', { name: adaName })
    await expect(card.getByText('Your seat', { exact: true })).toBeVisible()
    await expect(card.getByRole('button', { name: /^Join/ })).toHaveCount(0)
    await expect(page.getByRole('link', { name: /^Back to your lobby/ })).toHaveAttribute('href', lobby)

    // Anybody else finds it full.
    await register(bo.page, unique('bo'))
    const theirs = bo.page.getByRole('article', { name: adaName })
    await expect(theirs.getByText('Full', { exact: true })).toBeVisible()
    await expect(theirs.getByRole('button', { name: /^Join/ })).toHaveAttribute('aria-disabled', 'true')

    // Open leads back to the lobby, on the same seat.
    await card.getByRole('link', { name: `Open your seat in ${adaName}'s game` }).click()
    await expect(page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    expect(new URL(page.url()).pathname).toBe(lobby)
    await expect(page.getByRole('button', { name: 'Ready', exact: true })).toBeVisible()
    await page.goto('/')
    await page.getByRole('link', { name: /^Back to your lobby/ }).click()
    await expect(page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()

    for (const { errors } of [ada, bo]) expect(errors).toEqual([])
  } finally {
    for (const { context } of [ada, bo]) await context.close()
  }
})
