// Goal: a tournament runs in the browser on a real server and database: an organiser creates a Swiss tournament, two players sign up through the invite link, the organiser starts round 1, each player finds "Play your game" and opens the match game, which gives the player no host controls, the organiser enters the result, and finishing the tournament shows the final standings; a player sees no organiser controls.
import { expect, test } from '@playwright/test'
import type { Browser, BrowserContext, Page } from '@playwright/test'
import { ACCOUNTS_DATABASE_URL, ACCOUNTS_WEB_URL } from './ports'

// Accounts need Postgres, so this spec runs only with E2E_DATABASE_URL, like `accounts.spec.ts`.
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

const organiser = (page: Page) => page.getByRole('region', { name: 'Organiser' })
const roundOne = (page: Page) => page.getByRole('region', { name: 'Round 1' })

test('an organiser runs a Swiss tournament of two players from sign-up to final standings', async ({
  browser,
}) => {
  test.setTimeout(120_000)
  const [org, ann, bob] = [await visitor(browser), await visitor(browser), await visitor(browser)]
  try {
    const names = { org: unique('org'), ann: unique('ann'), bob: unique('bob') }
    await register(org.page, names.org)
    await org.page.goto('/tournaments')
    const form = org.page.getByRole('form', { name: 'New tournament' })
    await form.getByLabel('Name').fill('Friday Cup')
    await form.getByLabel('Format').selectOption('commander_1v1')
    await form.getByLabel('Rounds are paired by').selectOption('swiss')
    await form.getByLabel('Rounds', { exact: true }).selectOption('1')
    await form.getByRole('button', { name: 'Create tournament' }).click()
    await expect(org.page.getByRole('heading', { level: 1, name: 'Friday Cup' })).toBeVisible()
    const invite = (await organiser(org.page).locator('code').textContent()) ?? ''
    expect(invite).toContain('/tournaments/join/')

    // Both players sign up through the link and land on the tournament, without organiser controls.
    for (const [player, name] of [
      [ann, names.ann],
      [bob, names.bob],
    ] as const) {
      await register(player.page, name)
      await player.page.goto(new URL(invite).pathname)
      await player.page
        .getByRole('form', { name: 'Sign up' })
        .getByRole('button', { name: 'Sign up' })
        .click()
      await expect(player.page.getByRole('heading', { level: 1, name: 'Friday Cup' })).toBeVisible()
      await expect(organiser(player.page)).toHaveCount(0)
    }

    // The organiser starts round 1; each player finds their game and opens it.
    await org.page.reload()
    await organiser(org.page).getByRole('button', { name: 'Start round 1' }).click()
    await expect(roundOne(org.page)).toBeVisible()
    for (const player of [ann, bob]) {
      await player.page.reload()
      const play = roundOne(player.page).getByRole('link', { name: 'Play your game' })
      await expect(play).toBeVisible()
      await play.click()
      await expect(player.page).toHaveURL(/\/game\/[0-9a-f-]+/)
      // A match game has no player host: nobody closes the table or changes its teams.
      await expect(player.page.getByRole('button', { name: /close (the )?(game|table)/i })).toHaveCount(0)
    }

    // The organiser enters the result and finishes the tournament: the winner leads the standings.
    await org.page.reload()
    await roundOne(org.page)
      .getByLabel('Result of table 1')
      .selectOption({ label: `${names.ann} won` })
    await organiser(org.page).getByRole('button', { name: 'Finish tournament' }).click()
    await expect(org.page.getByText('Finished', { exact: true }).first()).toBeVisible()
    const standings = org.page.getByRole('region', { name: 'Final standings' })
    const rows = standings.getByRole('row')
    await expect(rows.nth(1)).toContainText(names.ann)
    await expect(rows.nth(2)).toContainText(names.bob)

    for (const { errors } of [org, ann, bob]) expect(errors).toEqual([])
  } finally {
    for (const { context } of [org, ann, bob]) await context.close()
  }
})
