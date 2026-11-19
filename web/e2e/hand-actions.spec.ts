// Goal: a hand card that can be cast or typecycled (Angel of the Ruins) names both plays with their costs and effects in its chooser, and the payment of the Plainscycling names what it pays for and can be cancelled with the card left in the hand and nothing tapped; in a duel and at four seats, at 1400x900 and on a phone (390x844).
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

/** The viewer's Angel of the Ruins in hand. */
const angel = (page: Page) =>
  page
    .getByRole('region', { name: 'Your hand' })
    .locator('button[data-object-id][aria-label^="Angel of the Ruins"]')
    .first()

for (const [width, height] of [
  [1400, 900],
  [390, 844],
] as const) {
  for (const seats of [2, 4]) {
    test.describe(`a hand card with two plays at ${seats} seats, ${width}x${height}`, () => {
      test.use({ viewport: { width, height } })

      test('the chooser names the cast and the Plainscycling with their costs and effects', async ({
        page,
      }) => {
        await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}&prompt=priority-cycling`)
        await angel(page).click()
        const chooser = page.locator('[data-slot="ability-chooser"]')
        await expect(chooser).toBeVisible()
        const cast = chooser.locator('[data-play="cast"]')
        await expect(cast).toContainText('Cast Angel of the Ruins')
        await expect(cast).toContainText('{5}{W}{W} · Artifact Creature — Angel 5/7')
        const cycling = chooser.getByRole('menuitem', { name: 'Activate Plainscycling 2' })
        await expect(cycling).toContainText('Plainscycling {2}')
        await expect(cycling).toContainText('{2}, Discard this card: Search your library for a Plains card')
        // Both plays fit the window.
        const box = await chooser.boundingBox()
        expect(box).not.toBeNull()
        if (box !== null) expect(box.x + box.width).toBeLessThanOrEqual(width)
        await page.screenshot({ path: `test-results/hand-choice-${seats}-${width}.png` })
      })

      test('the Plainscycling payment names what it pays for, and Cancel leaves the card in the hand with nothing paid', async ({
        page,
      }) => {
        await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}&prompt=pay-cycling`)
        const chip = page.locator('[data-slot="prompt-asker"]').first()
        await expect(chip.locator('[data-slot="asker-doing"]')).toHaveText('Plainscycling')
        await expect(chip).toContainText('Angel of the Ruins')
        await expect(page.locator('[data-slot="pay-meter"]').first()).toHaveAttribute(
          'aria-label',
          '0 of 2 paid',
        )
        await page.screenshot({ path: `test-results/pay-cycling-${seats}-${width}.png` })
        // A source picked, then the activation cancelled: nothing was paid, so nothing stays tapped.
        await page.getByRole('button', { name: 'Pay for me', exact: true }).click()
        await expect(page.locator('[data-slot="pay-meter"]').first()).toHaveAttribute(
          'aria-label',
          '2 of 2 paid',
        )
        await page.getByRole('button', { name: 'Cancel the ability' }).click()
        await expect(page.locator('[data-decision]')).toHaveCount(0)
        await expect(angel(page)).toBeVisible()
        await expect(
          page.locator(`section[aria-label$="'s battlefield"] button[data-object-id][aria-pressed="true"]`),
        ).toHaveCount(0)
      })
    })
  }
}
