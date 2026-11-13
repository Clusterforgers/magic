// Goal: on a real server and database a signed-in stranger watches a running game of two accounts with no seat and no hand of its own, the game ends when a seat concedes and the watcher is told, and the finished game replays to a seat while a visitor who is not signed in is asked to sign in.
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

/** Creates a two-seat Normal game with a link for every seat as the signed-in user; answers the links. */
async function createGame(page: Page): Promise<string[]> {
  await page.goto('/new')
  await page.getByRole('radio', { name: /^Normal/ }).check()
  await expect(page.getByRole('form', { name: 'Create a game' })).toBeVisible()
  await page.getByText('Advanced', { exact: true }).click()
  await page.getByRole('checkbox', { name: 'Give every seat its own link' }).check()
  await page.getByRole('button', { name: 'Create game' }).click()
  await expect(page.getByRole('heading', { name: 'Game created' })).toBeVisible()
  const links = page.getByRole('list', { name: 'Seat links' }).locator('a[data-share-link]')
  await expect(links).toHaveCount(2)
  return links.evaluateAll((anchors) => anchors.map((anchor) => anchor.getAttribute('href') ?? ''))
}

test('a stranger watches a running game to its end, and the finished game replays to its seats only', async ({
  browser,
}) => {
  test.setTimeout(120_000)
  const [ada, bo, eve, guest] = [
    await visitor(browser),
    await visitor(browser),
    await visitor(browser),
    await visitor(browser),
  ]
  try {
    const adaName = unique('ada')
    await register(ada.page, adaName)
    const links = await createGame(ada.page)
    await ada.page.getByRole('button', { name: `Open my seat (${adaName})` }).click()
    await expect(ada.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    const game = new URL(ada.page.url()).pathname.split('/').at(-1) ?? ''
    await register(bo.page, unique('bo'))
    await bo.page.goto(links[1] ?? '')
    await expect(bo.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    for (const { page } of [ada, bo]) await page.getByRole('button', { name: 'Ready', exact: true }).click()
    for (const { page } of [ada, bo]) await page.getByRole('button', { name: 'Keep', exact: true }).click()
    for (const { page } of [ada, bo]) {
      await expect(page.getByRole('heading', { name: 'Opening hands' })).toHaveCount(0)
    }

    // A signed-in stranger watches with no seat; nobody signed out does.
    await register(eve.page, unique('eve'))
    await eve.page.goto(`/watch/${game}`)
    await expect(eve.page.getByText('Watching: you hold no seat')).toBeVisible()
    await expect(eve.page.getByRole('region', { name: 'Game table' })).toBeVisible()
    await expect(eve.page.getByRole('button', { name: 'Keep', exact: true })).toHaveCount(0)
    await guest.page.goto(`/watch/${game}`)
    await expect(guest.page.getByRole('heading', { name: 'Sign in to watch this game' })).toBeVisible()

    // Bo concedes; both seats and the watcher see the end.
    await bo.page.getByRole('button', { name: 'Table menu' }).click()
    await bo.page.getByRole('button', { name: 'Concede', exact: true }).click()
    await bo.page.getByRole('alertdialog').getByRole('button', { name: 'Concede', exact: true }).click()
    for (const { page } of [ada, bo, eve]) {
      await expect(page.locator('[data-end]')).toBeVisible({
        timeout: 15_000,
      })
    }

    // The replay opens for a seat and asks a guest to sign in.
    await ada.page.goto(`/replay/${game}`)
    await expect(ada.page.getByLabel('Replay', { exact: true })).toBeVisible()
    await guest.page.goto(`/replay/${game}`)
    await expect(guest.page.getByText('Sign in to watch replays')).toBeVisible()

    for (const { errors } of [ada, bo, eve, guest]) expect(errors).toEqual([])
  } finally {
    for (const { context } of [ada, bo, eve, guest]) await context.close()
  }
})
