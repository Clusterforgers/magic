// Goal: a payment takes a mana source pressed on the board at once, the moment the prompt is drawn (a basic land, a land of any colour, a land of two colours, Sol Ring and a Signet), so the card reads pressed, a source of several kinds lights its row and the circles fill; and "Pay for me" still answers; in a duel and at four seats, at 1400x900 and on a phone (390x844, the detail in the sheet).
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

/** The viewer's card on the board whose spoken name starts with `name`. */
const onBoard = (page: Page, name: string) =>
  page
    .locator(
      `section[aria-label$="'s battlefield"] button[data-object-id][data-highlight][aria-label^="${name},"]`,
    )
    .first()

const meter = (page: Page) => page.locator('[data-slot="pay-meter"]').first()

/** A press, then the colour asked for in the lit row when the source makes more than one kind. */
async function press(page: Page, name: string, colour?: string): Promise<void> {
  // A real pointer press on the card, at once: no wait for the table to settle first.
  await onBoard(page, name).click({ position: { x: 10, y: 6 } })
  if (colour !== undefined) {
    const lit = page.locator('[data-slot="pay-source-row"][data-lit]')
    await lit.getByRole('button', { name: `Add ${colour} mana` }).click()
  }
  await expect(onBoard(page, name)).toHaveAttribute('aria-pressed', 'true')
}

const SOURCES: readonly [string, string | undefined, string][] = [
  ['Command Tower', 'green', '1 of 4 paid'],
  ['Azorius Chancery', undefined, '2 of 4 paid'],
]

for (const [width, height] of [
  [1400, 900],
  [390, 844],
] as const) {
  for (const seats of [2, 4]) {
    test.describe(`a payment at ${seats} seats, ${width}x${height}`, () => {
      test.use({ viewport: { width, height } })

      test.beforeEach(async ({ page }) => {
        await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}&prompt=pay-sources`)
        await meter(page).waitFor({ state: 'attached' })
      })

      test('takes a basic land pressed on the board at once, and gives it back on a second press', async ({
        page,
      }) => {
        const basic = page
          .locator(
            `section[aria-label$="'s battlefield"] button[data-highlight="legal"][aria-label*="Basic Land"]`,
          )
          .first()
        const id = await basic.getAttribute('data-object-id')
        const card = page.locator(`button[data-object-id="${id}"]`)
        await card.click({ position: { x: 10, y: 6 } })
        await expect(card).toHaveAttribute('aria-pressed', 'true')
        await expect(meter(page)).toHaveAttribute('aria-label', '1 of 4 paid')
        await card.click({ position: { x: 10, y: 6 } })
        await expect(card).toHaveAttribute('aria-pressed', 'false')
        await expect(meter(page)).toHaveAttribute('aria-label', '0 of 4 paid')
      })

      for (const [name, colour, paid] of SOURCES) {
        test(`takes ${name} pressed on the board`, async ({ page }) => {
          await press(page, name, colour)
          await expect(meter(page)).toHaveAttribute('aria-label', paid)
        })
      }

      test('takes Sol Ring, then a Signet whose cost it pays', async ({ page }) => {
        await press(page, 'Sol Ring')
        await press(page, 'Orzhov Signet')
        await expect(page.locator('[data-slot="pay-source-row"][data-used]')).toHaveCount(2)
      })

      test('still pays with "Pay for me"', async ({ page }) => {
        await page.getByRole('button', { name: 'Pay for me', exact: true }).click()
        await expect(page.locator('[data-decision]')).toHaveCount(0)
      })
    })
  }
}
