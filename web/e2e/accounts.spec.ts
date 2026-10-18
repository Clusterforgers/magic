// Goal: accounts work end to end in a browser on a real server and database: register, create a game, come back through My games and rejoin with no seat link, two accounts at one table, and signing out and in again.
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

async function register(page: Page, username: string): Promise<void> {
  await page.goto('/register')
  await page.getByLabel('Username').fill(username)
  await page.getByLabel('Password').fill(PASSWORD)
  await page.getByRole('button', { name: 'Create account' }).click()
  // Registering signs in and goes home, which is the create page.
  await expect(page.getByRole('form', { name: 'Create a game' })).toBeVisible()
  await expect(account(page).getByText(username)).toBeVisible()
}

/** Creates a two-seat game on `/` as the signed-in user and returns the seat links, `/game/<id>#seat=<n>&token=<t>`. */
async function createGame(page: Page): Promise<string[]> {
  await page.getByRole('button', { name: 'Create game' }).click()
  await expect(page.getByRole('heading', { name: 'Game created' })).toBeVisible()
  const links = page.getByRole('list', { name: 'Seat links' }).locator('a[data-share-link]')
  await expect(links).toHaveCount(2)
  return links.evaluateAll((anchors) => anchors.map((anchor) => anchor.getAttribute('href') ?? ''))
}

const games = (page: Page) => page.getByRole('list', { name: 'Your games' }).getByRole('listitem')
const gameIdOf = (page: Page): string => new URL(page.url()).pathname.split('/').at(-1) ?? ''

test('an account creates a game, closes the page, finds it under My games and rejoins it without a link', async ({
  browser,
}) => {
  const ada = unique('ada')
  const { context, page, errors } = await visitor(browser)
  try {
    await register(page, ada)
    await expect(page.locator('[data-slot="signed-in-note"]')).toContainText(
      `You will play as ${ada} in seat 1`,
    )

    await createGame(page)
    await expect(page.locator('[data-slot="saved-note"]')).toContainText('saved to your account')
    await page.getByRole('button', { name: `Open my seat (${ada})` }).click()
    await expect(page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    await expect(page.locator('[data-slot="seat-saved"]')).toContainText('This seat is saved to your account')
    const game = gameIdOf(page)
    await page.close()

    // A new page of the same browser: the cookie is all that is left of the first.
    const back = await context.newPage()
    back.on('pageerror', (error) => errors.push(error.message))
    await back.goto('/games')
    await expect(games(back)).toHaveCount(1)
    const row = games(back).first()
    await expect(row).toHaveAttribute('data-game', game)
    await expect(row).toHaveAttribute('data-status', 'lobby')
    await expect(row).toContainText('Lobby')
    await expect(row).toContainText('With Player 2')
    await expect(row).toContainText(`You play ${ada} in seat 1.`)

    await row.getByRole('link', { name: /rejoin/i }).click()
    await expect(back.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    // No seat link: the address is the game and nothing else, and the page is connected all the same.
    expect(new URL(back.url()).pathname).toBe(`/game/${game}`)
    expect(new URL(back.url()).hash).toBe('')
    await expect(back.getByRole('list', { name: 'Seats' }).getByRole('listitem').first()).toContainText('you')
    await expect(back.getByText('1 of 2 here')).toBeVisible()
    expect(errors).toEqual([])
  } finally {
    await context.close()
  }
})

test('a second account joins by seat link, and both accounts list the game, in the lobby and once it runs', async ({
  browser,
}) => {
  const adaName = unique('ada')
  const boName = unique('bo')
  const ada = await visitor(browser)
  const bo = await visitor(browser)
  try {
    await register(ada.page, adaName)
    const links = await createGame(ada.page)
    await ada.page.getByRole('button', { name: `Open my seat (${adaName})` }).click()
    await expect(ada.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    const game = gameIdOf(ada.page)

    // Bo opens seat 2's link while signed in: the server saves that seat to bo's account.
    await register(bo.page, boName)
    const link = links[1]
    if (link === undefined || !link.startsWith('/game/')) throw new Error(`no link for seat 2: ${link}`)
    await bo.page.goto(link)
    await expect(bo.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    await expect(bo.page.locator('[data-slot="seat-saved"]')).toContainText(
      'This seat is saved to your account',
    )
    // Ada's lobby learns who sits where when bo connects (it polls every 3 s).
    const seats = ada.page.getByRole('list', { name: 'Seats' }).getByRole('listitem')
    await expect(seats.nth(1)).toContainText(`signed in as ${boName}`, { timeout: 15_000 })
    await expect(seats.nth(0)).toContainText(`signed in as ${adaName}`)

    // Both lists have the game, each naming the other account.
    await ada.page.goto('/games')
    await bo.page.goto('/games')
    await expect(games(ada.page)).toHaveCount(1)
    await expect(games(bo.page)).toHaveCount(1)
    await expect(games(ada.page).first()).toHaveAttribute('data-game', game)
    await expect(games(bo.page).first()).toHaveAttribute('data-game', game)
    await expect(games(ada.page).first()).toContainText(`With ${boName}`)
    await expect(games(bo.page).first()).toContainText(`With ${adaName}`)
    await expect(games(bo.page).first()).toContainText(`You play Player 2 in seat 2.`)

    // Start it from the two lobbies, which both lists then show as running.
    await ada.page.getByRole('link', { name: /rejoin/i }).click()
    await bo.page.getByRole('link', { name: /rejoin/i }).click()
    for (const { page } of [ada, bo]) {
      await page.getByRole('button', { name: 'Ready', exact: true }).click()
    }
    for (const { page } of [ada, bo]) {
      await expect(page.getByRole('heading', { name: 'Opening hands' })).toBeVisible()
    }
    for (const { page } of [ada, bo]) {
      await page.goto('/games')
      await expect(games(page).first()).toHaveAttribute('data-status', 'running')
      await expect(games(page).first()).toContainText('Running')
    }

    // Back into the running game by the session alone, on both sides.
    for (const { page } of [ada, bo]) {
      await games(page)
        .first()
        .getByRole('link', { name: /rejoin/i })
        .click()
      await expect(page.getByRole('heading', { name: 'Opening hands' })).toBeVisible()
      expect(new URL(page.url()).hash).toBe('')
    }
    expect([...ada.errors, ...bo.errors]).toEqual([])
  } finally {
    await Promise.all([ada.context.close(), bo.context.close()])
  }
})

test('signing out leaves a guest, My games asks for a sign-in and comes back to the list after it', async ({
  browser,
}) => {
  const adaName = unique('ada')
  const { context, page, errors } = await visitor(browser)
  try {
    await register(page, adaName)
    await createGame(page)
    await page.goto('/games')
    await expect(games(page)).toHaveCount(1)

    await account(page).getByRole('button', { name: 'Sign out' }).click()
    await expect(page.getByRole('form', { name: 'Create a game' })).toBeVisible() // signing out on /games goes home
    await expect(account(page).getByRole('link', { name: 'Sign in' })).toBeVisible()
    await expect(account(page).getByText(adaName)).toHaveCount(0)

    await page.goto('/games')
    await expect(page.getByRole('form', { name: 'Sign in' })).toBeVisible()
    expect(new URL(page.url()).pathname).toBe('/login')
    expect(new URL(page.url()).searchParams.get('next')).toBe('/games')

    // A wrong password says so in one sentence and keeps the username; the right one goes back to the list.
    await page.getByLabel('Username').fill(adaName)
    await page.getByLabel('Password').fill('not the right one!')
    await page.getByRole('button', { name: 'Sign in' }).click()
    await expect(page.getByRole('alert')).toContainText('The username or password is wrong')
    await expect(page.getByLabel('Password')).toHaveValue('')
    await page.getByLabel('Password').fill(PASSWORD)
    await page.getByRole('button', { name: 'Sign in' }).click()
    await expect(games(page)).toHaveCount(1)
    expect(new URL(page.url()).pathname).toBe('/games')
    expect(errors).toEqual([])
  } finally {
    await context.close()
  }
})
