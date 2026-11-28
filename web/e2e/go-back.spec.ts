// Goal: against a bot, a seat that passed its precombat main phase gets "Back to Precombat main" at its next prompt, at desktop size and on a phone, and pressing it returns it to that main phase with the control gone and the log saying so.
import { expect, test } from './test'
import type { Page } from '@playwright/test'
import { logOf } from './manual'

const yourTurn = (page: Page) => page.locator('[data-slot="banner"][data-your-turn]')
const primary = (page: Page) => page.locator('[data-slot="primary-action"]')
const back = (page: Page) => page.getByRole('button', { name: 'Back to Precombat main' })

for (const viewport of [
  { width: 1400, height: 900 },
  { width: 390, height: 844 },
]) {
  test(`a seat goes back to its main phase against a bot at ${viewport.width} px`, async ({ browser }) => {
    test.setTimeout(3 * 60_000)
    const context = await browser.newContext({ viewport })
    const page = await context.newPage()
    const errors: string[] = []
    page.on('pageerror', (error) => errors.push(error.message))
    try {
      await page.goto('/new')
      await page.getByRole('radio', { name: /^Normal/ }).check()
      await page.getByLabel('Bots').selectOption('1')
      await page.getByRole('button', { name: 'Create game' }).click()
      await expect(page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
      await page.getByRole('button', { name: 'Ready', exact: true }).click()
      await page.getByRole('button', { name: 'Keep', exact: true }).click()

      // The bot may begin: the seat plays through its turn until its own begins.
      for (let press = 0; press < 60 && (await yourTurn(page).count()) === 0; press++) {
        const end = page.getByRole('button', { name: 'End turn', exact: true })
        if (await end.isVisible()) await end.click()
        else if (await primary(page).isVisible()) await primary(page).click()
        await page.waitForTimeout(300)
      }
      await expect(yourTurn(page)).toBeVisible()
      await expect(primary(page)).toContainText('Next', { timeout: 20_000 })
      await expect(back(page)).toHaveCount(0)

      // It passes its main phase by hand and is asked again later in the turn, where it may go back.
      await primary(page).click()
      await expect(back(page)).toBeVisible({ timeout: 20_000 })
      await expect(back(page)).toBeInViewport()
      await back(page).click()
      await expect(back(page)).toHaveCount(0)
      await logOf(page, [/went back to the precombat main phase/])
      expect(errors, 'page errors').toEqual([])
    } finally {
      await context.close()
    }
  })
}
