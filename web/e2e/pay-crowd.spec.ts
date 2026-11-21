// Goal: a payment on a crowded board (60 and 120 more mana sources of every kind: basics, duals, rocks, Signets, Treasures, filter lands, Cabal Coffers, creature-only mana) opens within about a second and stays quick: a source pressed on the board reads pressed, "Pay for me" lays out the game's payment and Confirm pays it; in a duel and at four seats, at 1400x900 and on a phone (390x844).
import { expect, test } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const meter = '[data-slot="pay-meter"]'

for (const [width, height] of [
  [1400, 900],
  [390, 844],
] as const) {
  for (const seats of [2, 4]) {
    for (const prompt of ['pay-crowd', 'pay-crowd-120']) {
      test(`${prompt} at ${seats} seats, ${width}x${height} stays quick`, async ({ page }) => {
        test.setTimeout(60_000)
        await page.setViewportSize({ width, height })
        // The table first, then the payment: the time to the pay step is the prompt's own work.
        await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}`)
        await page.locator('section[aria-label$="\'s battlefield"]').first().waitFor()
        const opened = await page.evaluate(
          async ([url, slot]) => {
            const start = performance.now()
            window.history.pushState({}, '', url)
            window.dispatchEvent(new PopStateEvent('popstate'))
            while (document.querySelector(slot) === null) {
              await new Promise((done) => requestAnimationFrame(done))
              if (performance.now() - start > 20_000) break
            }
            return performance.now() - start
          },
          [`/dev/table?seats=${seats}&prompt=${prompt}`, meter] as const,
        )
        expect(opened).toBeLessThan(prompt === 'pay-crowd' ? 1_000 : 2_000)

        const legal = page
          .locator(
            `section[aria-label$="'s battlefield"] button[data-highlight="legal"][aria-label*="Basic Land"]`,
          )
          .first()
        const id = await legal.getAttribute('data-object-id')
        const card = page.locator(`button[data-object-id="${id}"]`)
        let start = Date.now()
        await card.click({ position: { x: 10, y: 6 } })
        await expect(card).toHaveAttribute('aria-pressed', 'true')
        expect(Date.now() - start).toBeLessThan(1_000)
        await card.click({ position: { x: 10, y: 6 } })
        await expect(card).toHaveAttribute('aria-pressed', 'false')

        start = Date.now()
        await page.getByRole('button', { name: 'Pay for me', exact: true }).click()
        const confirm = page.getByRole('button', { name: 'Confirm payment' })
        await expect(confirm).toBeEnabled()
        expect(Date.now() - start).toBeLessThan(1_000)
        await confirm.click()
        await expect(page.locator('[data-decision]')).toHaveCount(0)
      })
    }
  }
}
