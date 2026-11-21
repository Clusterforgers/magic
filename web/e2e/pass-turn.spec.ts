// Goal: "Pass until my turn" beside "Auto-skip triggers" is turned on by the seat whose turn it is not, shows the line that says why the game moves on, keeps that seat free of every priority prompt while the other seat plays its turn through, and is off again, line gone, once the seat's own turn begins.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { createAndJoin, everyoneReady } from './lobby'

const passTurn = (page: Page) => page.getByRole('switch', { name: 'Pass until my turn' })
const line = (page: Page) => page.locator('[data-slot="yield-indicator"]')
const yourTurn = (page: Page) => page.locator('[data-slot="banner"][data-your-turn]')
const primary = (page: Page) => page.locator('[data-slot="primary-action"]')

test('Pass until my turn passes through the other turn and turns itself off at the own turn', async ({
  browser,
}) => {
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
    await expect(yourTurn(ada.page).or(yourTurn(bo.page))).toBeVisible()
    const activeFirst = (await yourTurn(ada.page).count()) > 0
    const [active, waiting] = activeFirst ? [ada, bo] : [bo, ada]

    // The switch shows what the server holds: on once it says the seat passes, with the line that says so.
    await expect(passTurn(waiting.page)).not.toBeChecked()
    await passTurn(waiting.page).click()
    await expect(passTurn(waiting.page)).toBeChecked()
    await expect(line(waiting.page)).toContainText('Passing until your turn')
    await expect(line(waiting.page)).toHaveAttribute('data-yield', 'MyTurnThrough')

    // The active seat plays its turn out; the waiting seat is never given a priority prompt meanwhile.
    for (let press = 0; press < 40 && (await yourTurn(waiting.page).count()) === 0; press++) {
      expect(await primary(waiting.page).count(), 'a priority prompt while passing').toBe(0)
      const end = active.page.getByRole('button', { name: 'End turn', exact: true })
      if (await end.isVisible()) await end.click()
      else if (await primary(active.page).isVisible()) await primary(active.page).click()
      await active.page.waitForTimeout(300)
    }
    await expect(yourTurn(waiting.page)).toBeVisible()

    // Its own turn turned the switch off and took the line away.
    await expect(passTurn(waiting.page)).not.toBeChecked()
    await expect(line(waiting.page)).toHaveCount(0)
    for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
