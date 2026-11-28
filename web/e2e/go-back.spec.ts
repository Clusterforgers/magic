// Goal: a seat that passed its precombat main phase while the table passed for the other seat gets "Back to Precombat main" at its next prompt, and pressing it returns it to that main phase, the control gone and the other seat's log saying so.
import { expect, test } from './test'
import type { Page } from '@playwright/test'
import { createAndJoin, everyoneReady } from './lobby'
import { logOf } from './manual'

const passTurn = (page: Page) => page.getByRole('switch', { name: 'Pass until my turn' })
const yourTurn = (page: Page) => page.locator('[data-slot="banner"][data-your-turn]')
const primary = (page: Page) => page.locator('[data-slot="primary-action"]')
const back = (page: Page) => page.getByRole('button', { name: 'Back to Precombat main' })

test('a seat goes back to its main phase and the table reads it in the log', async ({ browser }) => {
  test.setTimeout(3 * 60_000)
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  try {
    const [ada, bo] = players
    if (ada === undefined || bo === undefined) throw new Error('two players expected')
    await everyoneReady(players)
    await Promise.all(
      players.map(({ page }) => page.getByRole('button', { name: 'Keep', exact: true }).click()),
    )
    await expect
      .poll(async () => (await yourTurn(ada.page).count()) + (await yourTurn(bo.page).count()), {
        timeout: 30_000,
      })
      .toBe(1)
    const activeFirst = (await yourTurn(ada.page).count()) > 0
    const [active, waiting] = activeFirst ? [ada, bo] : [bo, ada]

    // The table passes for the waiting seat, so its passes never stand in the way.
    await passTurn(waiting.page).click()
    await expect(passTurn(waiting.page)).toBeChecked()

    // The active seat passes its first main phase by hand and is asked again later in the turn.
    await expect(primary(active.page)).toContainText('Next')
    await expect(back(active.page)).toHaveCount(0)
    await primary(active.page).click()
    await expect(back(active.page)).toBeVisible({ timeout: 20_000 })

    await back(active.page).click()
    await expect(back(active.page)).toHaveCount(0)
    await expect(active.page.locator('[data-slot="banner"]')).toContainText(/precombat main/i)
    await logOf(waiting.page, [/went back to the precombat main phase/])
    for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
