// Goal: from the Play page a new player reaches the Learn page, starts the guided game against the practice bot, and the lesson moves on by itself as they play a land and cast a creature, while the table under the coach stays usable; the lesson can be skipped.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { WEB_URL } from './ports'

const coach = (page: Page) => page.locator('[data-slot="lesson-coach"] [data-slot="tour-card"]')
const handCard = (page: Page, name: string) =>
  page.locator('[data-region="hand"] [data-object-id]').filter({ hasText: name }).first()

test('the guided game moves on with the player’s own moves', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 })
  await page.goto(`${WEB_URL}/`)
  await page.locator('[data-slot="learn-card"]').getByRole('link', { name: 'Tutorials' }).click()
  await expect(page.getByRole('heading', { name: 'Learn', level: 1 })).toBeVisible()
  await page
    .getByRole('listitem', { name: /Playing a game/ })
    .getByRole('button', { name: 'Start' })
    .click()

  await expect(page.locator('[data-region="hand"]')).toBeVisible()
  await expect(coach(page)).toHaveAttribute('aria-labelledby', /.+/)
  await expect(coach(page).getByRole('heading', { name: 'Your first game' })).toBeVisible()
  await coach(page).getByRole('button', { name: 'Next' }).click()

  await expect(coach(page).getByRole('heading', { name: 'Play a land' })).toBeVisible()
  await expect(coach(page).getByText('Waiting for your land')).toBeVisible()
  await expect(page.locator('[data-slot="tour-spotlight"]')).toHaveCount(1)
  await handCard(page, 'Forest').click()

  await expect(coach(page).getByRole('heading', { name: 'Cast a creature' })).toBeVisible()
  await handCard(page, 'Grizzly Bears').click()
  await page.getByRole('button', { name: 'Pay automatically' }).click()

  await expect(coach(page).getByRole('heading', { name: 'Pass priority' })).toBeVisible()
  await expect(coach(page).getByText('Step 4 of')).toBeVisible()
  await coach(page).getByRole('button', { name: 'Skip lesson' }).click()
  await expect(coach(page)).toHaveCount(0)
  await expect(page.locator('[data-region="hand"]')).toBeVisible()
})
