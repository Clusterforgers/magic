// Goal: a one-player Delves run with bot allies goes from the lobby through the first boss fight to the loot screen on a real server and database: the player creates a delve, adds two bot allies, every hero takes a precon, Descend leads to the first boss, Enter the fight opens the boss's waiting room, Ready to fight starts the table, the boss's concession (the server's test hook) gives the party the win, the table returns to the run by itself and the run offers the relic pick, then the card pick.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import type { PreconListItem } from '../src/protocol/generated'
import { ACCOUNTS_DATABASE_URL } from './ports'
import { precons, signUp, sit } from './table'

// Delves need the card data and the precons (`mtg-cli ingest`, or `create database … template magic`), so this spec
// runs only with `E2E_DATABASE_URL`, like `precon-game.spec.ts`; the config starts that server with `MTG_TEST_HOOKS`.
test.skip(
  ACCOUNTS_DATABASE_URL === undefined,
  'set E2E_DATABASE_URL to a database with cards and precons to run this',
)

/** Takes `deck` for `hero` in the lobby's deck picker; the search opens the precons whatever the picker shows first. */
async function takeDeck(page: Page, hero: string, deck: PreconListItem): Promise<void> {
  await page.getByRole('button', { name: `Choose a deck for ${hero}` }).click()
  const dialog = page.getByRole('dialog', { name: `Choose a deck for ${hero}` })
  await dialog.getByLabel('Search decks').fill(deck.name)
  await dialog.locator(`[data-deck="precon:${deck.code}"]`).click()
  await dialog.getByRole('button', { name: `Take ${deck.name} for ${hero}` }).click()
  await expect(dialog).toBeHidden()
  await expect(page.getByRole('listitem', { name: new RegExp(`^${hero}.*: ready with `) })).toBeVisible()
}

test('a hero with two bot allies descends, fights the first boss and reaches the loot', async ({
  browser,
}) => {
  test.setTimeout(5 * 60_000)
  const account = await signUp(browser, 'delver')
  const { verified } = await precons(browser)
  const deck = verified[0]
  expect(deck, 'the database has no playable Commander precon').toBeDefined()
  const hero = await sit(browser, account)
  const { page } = hero
  try {
    // A new delve with a pinned ladder, and the party: the player and two bot allies.
    await page.goto('/delves')
    await page.getByRole('form', { name: 'New delve' }).getByLabel('Seed code (optional)').fill('E2E')
    await page.getByRole('button', { name: 'Create delve' }).click()
    await expect(page).toHaveURL(/\/delves\/[0-9a-f-]{36}$/)
    const run = new URL(page.url()).pathname.split('/').at(-1) ?? ''
    await expect(page.getByRole('heading', { name: 'Gather the party' })).toBeVisible()
    for (const ally of ['Ally 1', 'Ally 2']) {
      await page.getByRole('button', { name: 'Add a bot ally' }).click()
      await expect(page.getByRole('button', { name: `Choose a deck for ${ally}` })).toBeVisible()
    }
    for (const name of [account.name, 'Ally 1', 'Ally 2']) await takeDeck(page, name, deck as PreconListItem)
    await expect(page.getByText('3 of 3 heroes ready')).toBeVisible()

    // Descend: the first boss's intro and the way into its table.
    await page.getByRole('button', { name: 'Descend' }).click()
    await expect(page.getByRole('region', { name: 'Boss fight' })).toBeVisible()
    await page.getByRole('link', { name: 'Enter the fight' }).click()
    await expect(page).toHaveURL(/\/game\/[0-9a-f-]{36}$/)
    await page.getByRole('button', { name: 'Ready to fight' }).click()

    // The fight is a real game; the boss concedes it as soon as the table runs, so the party wins without playing it.
    const concede = `/api/test/delves/${run}/boss-concedes`
    await expect
      .poll(async () => (await page.request.post(concede)).status(), {
        message: 'the table starts and the boss concedes',
        timeout: 90_000,
        intervals: [250, 500, 1_000],
      })
      .toBe(204)

    // The game page hands the party back to the run, which reads the win and offers the loot.
    await expect(page).toHaveURL(new RegExp(`/delves/${run}$`), { timeout: 60_000 })
    const relics = page.getByRole('region', { name: 'Relic pick' })
    await expect(relics).toBeVisible({ timeout: 30_000 })
    await expect(page.getByText(/falls! Each hero heals 10/)).toBeVisible()
    await relics
      .getByRole('button', { name: /^Take (?!none)/ })
      .first()
      .click()
    await expect(page.getByRole('region', { name: 'Card pick' })).toBeVisible()
    expect(hero.errors, 'uncaught page errors').toEqual([])
  } finally {
    await hero.context.close()
  }
})
