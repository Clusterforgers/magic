// Goal: from the Play page a new player reaches the Learn page and plays the whole guided game against the practice bot by pressing exactly what each step names, on the bot's turns too: every step shows while the table waits for what it asks, and the lesson ends with the win; the lesson can be skipped and leaves the table usable.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { WEB_URL } from './ports'

const coach = (page: Page) => page.locator('[data-slot="lesson-coach"] [data-slot="tour-card"]')
const object = (page: Page, area: string, name: string) =>
  page.locator(`${area} [data-object-id]`).filter({ hasText: name }).first()
const handCard = (page: Page, name: string) => object(page, '[data-region="hand"]', name)
const mine = (page: Page, name: string) => object(page, '[data-area="own"]', name)
const primary = (page: Page) => page.locator('[data-slot="primary-action"]')

/** Waits for the coach's step `title`, which must be shown while the table waits for what it asks. */
async function step(page: Page, title: string): Promise<void> {
  await expect(coach(page).getByRole('heading', { name: title })).toBeVisible({ timeout: 20_000 })
}

/** Presses the primary button once its words match `label`, after checking the step names it with the same words. */
async function pass(page: Page, label: RegExp): Promise<void> {
  await expect(primary(page)).toHaveAttribute('data-label', label)
  const words = (await primary(page).getAttribute('data-label')) ?? ''
  await expect(coach(page)).toContainText(words)
  await primary(page).click()
}

/** Presses the prompt button `name` that the step names by its words `words`. */
async function press(page: Page, name: string, words: string): Promise<void> {
  await expect(coach(page)).toContainText(words)
  await page.getByRole('button', { name, exact: true }).click()
}

async function startLesson(page: Page): Promise<void> {
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
  await step(page, 'Your first game')
  await coach(page).getByRole('button', { name: 'Next' }).click()
}

test('the guided game is played to the win by pressing what each step names', async ({ page }) => {
  test.setTimeout(180_000)
  await startLesson(page)

  await step(page, 'Play a land')
  await expect(coach(page).getByText('Waiting for your land')).toBeVisible()
  await expect(page.locator('[data-slot="tour-spotlight"]')).toHaveCount(1)
  await handCard(page, 'Forest').click()
  await step(page, 'Cast a creature')
  await handCard(page, 'Grizzly Bears').click()
  await step(page, 'Pay the cost')
  await press(page, 'Pay automatically', 'Pay automatically')
  await step(page, 'End your turn')
  await pass(page, /^Next: End step$/)

  await step(page, 'Another land')
  await handCard(page, 'Plains').click()
  await step(page, 'Go to combat')
  await pass(page, /^Next: Combat$/)
  await step(page, 'Attack')
  await mine(page, 'Grizzly Bears').click()
  await press(page, 'Confirm attack', 'press Attack')
  await step(page, 'Let combat go on')
  await pass(page, /^Next: (Declare blockers|Blockers)$/)
  await step(page, 'Cast Hero in Training')
  await handCard(page, 'Hero in Training').click()
  await step(page, 'Pay with the Plains')
  await press(page, 'Pay automatically', 'Pay automatically')
  await step(page, 'The stack')
  await pass(page, /^Resolve( Hero in Training)?$/)
  await step(page, 'A trigger on the stack')
  await pass(page, /^Resolve( Hero in Training)?$/)
  await step(page, 'Keep mana open')
  await pass(page, /^Next: End step$/)

  await step(page, 'The bot attacks')
  await pass(page, /^Continue$/)
  await step(page, 'Block')
  await mine(page, 'Hero in Training').click()
  await press(page, 'Confirm blocks', 'press Block')
  await step(page, 'Respond with an instant')
  await handCard(page, 'Giant Growth').click()
  await step(page, 'Choose the target')
  await mine(page, 'Hero in Training').click()
  await press(page, 'Confirm targets', 'Confirm targets')
  await step(page, 'Pay with the open Forest')
  await press(page, 'Pay automatically', 'Pay automatically')

  await step(page, 'Your land for the turn')
  await handCard(page, 'Forest').click()
  await step(page, 'The command zone')
  await page
    .locator('[data-slot="hand-command"] [data-object-id]')
    .filter({ hasText: 'Giada' })
    .first()
    .click()
  await step(page, 'Pay for your commander')
  await press(page, 'Pay automatically', 'Pay automatically')
  await step(page, 'Win the game')
  await mine(page, 'Grizzly Bears').click()
  await mine(page, 'Hero in Training').click()
  await press(page, 'Confirm attack', 'press Attack')
  await step(page, 'Deal the damage')
  await pass(page, /^Next: (Declare blockers|Blockers)$/)
  await step(page, 'You won')
})

test('the lesson can be skipped and leaves the table usable', async ({ page }) => {
  await startLesson(page)
  await step(page, 'Play a land')
  await coach(page).getByRole('button', { name: 'Skip lesson' }).click()
  await expect(coach(page)).toHaveCount(0)
  await expect(page.locator('[data-region="hand"]')).toBeVisible()
})
