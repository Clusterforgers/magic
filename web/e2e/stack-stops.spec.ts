// Goal: a player picks a stack stops level from the button beside "Auto-skip triggers" in a live game, the button shows the level once the server holds it, and the level is still held after the page is loaded again.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { createAndJoin, everyoneReady } from './lobby'

const stackStops = (page: Page) => page.locator('[data-slot="stack-stops"]')
const level = (page: Page, name: string) => page.getByRole('menuitemradio', { name })

test('a stack stops level is set from the dock, held by the server and kept over a reload', async ({
  browser,
}) => {
  test.setTimeout(3 * 60_000)
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  try {
    const [ada] = players
    if (ada === undefined) throw new Error('two players expected')
    await everyoneReady(players)
    const page = ada.page
    await expect(stackStops(page)).toHaveAttribute('data-level', 'All')
    await expect(stackStops(page)).toHaveAccessibleName('Stack stops: every spell and ability')

    await stackStops(page).click()
    await expect(level(page, 'Every spell and ability')).toHaveAttribute('aria-checked', 'true')
    await level(page, 'Only what targets me or mine').click()
    // The button shows the server's word, so it changes once the server holds the level.
    await expect(stackStops(page)).toHaveAttribute('data-level', 'Aimed')
    await expect(stackStops(page)).toHaveAccessibleName('Stack stops: only what targets me or mine')

    await page.reload()
    await expect(stackStops(page)).toHaveAttribute('data-level', 'Aimed')
    await stackStops(page).click()
    await level(page, "Opponents' only").click()
    await expect(stackStops(page)).toHaveAttribute('data-level', 'Opponents')
    for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
