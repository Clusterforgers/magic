// Goal: the deck builder leads a Commander deck with a Partner pair, refuses a third commander that cannot lead with the deck check's reason, and narrows the search to the pair's colour identity, at a wide and a phone window, on the dev route (no server).
import { expect, test } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SIZES = [
  { width: 1400, height: 900 },
  { width: 390, height: 844 },
]

for (const size of SIZES) {
  test(`a Partner pair leads, a refused commander says why and the identity filter follows at ${size.width}x${size.height}`, async ({
    page,
  }) => {
    await page.setViewportSize(size)
    await page.goto(`${DEV_WEB_URL}/dev/builder?deck=partners`)
    const slot = page.getByRole('region', { name: /^Commander/ })
    await expect(slot.locator('[data-commander]')).toHaveCount(2)
    await expect(slot).toContainText('Tymna the Weaver')
    await expect(slot).toContainText('Thrasios, Triton Hero')
    await expect(slot).toContainText('White, Blue, Black and Green identity')

    if (size.width < 1024)
      await page.getByRole('group', { name: 'Show' }).getByRole('button', { name: 'Search' }).click()
    await page.getByRole('searchbox', { name: 'Search cards' }).fill('Goblin Guide')
    await page.getByRole('button', { name: 'Add Goblin Guide to another part of the deck' }).click()
    await page
      .getByRole('menuitem', { name: /commander/i })
      .first()
      .click()
    const refusal = page.getByRole('alert')
    await expect(refusal).toContainText('Goblin Guide cannot be added as a commander')
    await expect(refusal).toContainText('it is no legendary creature and does not say it can')

    const within = page.getByRole('button', { name: /^Within the commander’s colors/ })
    await expect(within).toHaveAttribute('aria-pressed', 'false')
    await within.click()
    await expect(within).toHaveAttribute('aria-pressed', 'true')
    await expect(page.getByRole('button', { name: 'Add Goblin Guide to main deck' })).toHaveCount(0)
  })
}
