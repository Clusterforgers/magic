// Goal: on the development table's reworked boss fight at 1920x1080, 1400x900, 1024x768 and a 390x844 phone, the phase pill stands in the top bar beside a turn badge that keeps its words in view, the bar keeps the height it has at a table without a boss, and the pill, a boss chip and a hero's relic each open their custom card in the card preview, whole inside the window, where a phase's steps move to the other phase.
import { expect, test } from '@playwright/test'
import type { Locator, Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SIZES = [
  { name: 'wide desktop', width: 1920, height: 1080 },
  { name: 'desktop', width: 1400, height: 900 },
  { name: 'tablet', width: 1024, height: 768 },
  { name: 'phone', width: 390, height: 844 },
] as const

async function open(page: Page, query: string): Promise<void> {
  await page.goto(`${DEV_WEB_URL}/dev/table?${query}`)
  await page.waitForFunction(() => document.querySelectorAll('[data-battlefield-seat]').length > 0)
  await page.waitForTimeout(400)
}

async function box(locator: Locator) {
  const found = await locator.boundingBox()
  if (found === null) throw new Error('not drawn')
  return found
}

/** The card preview is drawn whole inside the window. */
async function insideWindow(page: Page, preview: Locator): Promise<void> {
  const size = page.viewportSize()
  if (size === null) throw new Error('no viewport')
  const at = await box(preview)
  expect(at.x).toBeGreaterThanOrEqual(0)
  expect(at.y).toBeGreaterThanOrEqual(0)
  expect(at.x + at.width).toBeLessThanOrEqual(size.width + 0.5)
  expect(at.y + at.height).toBeLessThanOrEqual(size.height + 0.5)
}

for (const size of SIZES) {
  test.describe(`custom cards on the ${size.name}`, () => {
    test.beforeEach(async ({ page }) => {
      await page.setViewportSize({ width: size.width, height: size.height })
    })

    test('the phase pill sits in the top bar, which keeps its height and the turn badge its words', async ({
      page,
    }) => {
      await open(page, 'seats=3')
      const plain = (await box(page.locator('[data-region="bar"]'))).height
      await open(page, 'seats=3&boss=0&rework=2')
      const bar = page.locator('[data-region="bar"]')
      expect((await box(bar)).height).toBe(plain)
      const pill = bar.locator('[data-slot="phase-countdown"]')
      await expect(pill).toBeVisible()
      await expect(pill).toHaveAttribute('aria-label', /^Phase 1: The Nest Stirs\. Phase 2 in 5 rounds\.$/)
      const pillBox = await box(pill)
      const barBox = await box(bar)
      expect(pillBox.y).toBeGreaterThanOrEqual(barBox.y)
      expect(pillBox.y + pillBox.height).toBeLessThanOrEqual(barBox.y + barBox.height)
      // The pill's words are whole: nothing it says is cut.
      for (const words of await pill.locator('span:visible').all()) {
        expect(await words.evaluate((el) => el.scrollWidth <= el.clientWidth + 1)).toBe(true)
      }
      // The badge keeps room for words beside the pill, never squeezed to its icon.
      const badge = page.locator('[data-slot="turn-badge"] [data-slot="banner"]')
      expect((await box(badge)).width).toBeGreaterThanOrEqual(size.width < 640 ? 16 : 48)
      await expect(page.locator('[data-slot="boss-abilities"] [data-slot="phase-countdown"]')).toHaveCount(0)
    })

    test('the pill, a boss chip and a relic open their cards in the preview', async ({ page }) => {
      await open(page, 'seats=3&boss=0&rework=2')
      const preview = page.getByTestId('card-preview')
      await page.locator('[data-region="bar"] [data-slot="phase-countdown"]').click()
      await expect(preview).toHaveAttribute('data-custom', 'Phase')
      await expect(preview.getByRole('figure', { name: 'The Nest Stirs' })).toBeVisible()
      await insideWindow(page, preview)
      await preview.getByRole('button', { name: /Plague Tide/ }).click()
      await expect(preview.getByRole('figure', { name: 'Plague Tide' })).toBeVisible()
      await insideWindow(page, preview)
      await page.keyboard.press('Escape')
      await expect(preview).toHaveCount(0)

      await page.locator('[data-slot="boss-ability"][data-kind="innate"]').click()
      await expect(preview).toHaveAttribute('data-custom', 'Innate')
      await expect(preview.locator('[data-slot="preview-kicker"]')).toHaveText('Innate · Level III')
      await insideWindow(page, preview)
      await page.keyboard.press('Escape')

      const relic = page.locator('[data-slot="player-effect"]', { hasText: 'Lantern of Embers' }).first()
      if (await relic.isVisible()) {
        await relic.click()
        await expect(preview.getByRole('figure', { name: 'Lantern of Embers' })).toHaveAttribute(
          'data-rarity',
          'Rare',
        )
        await insideWindow(page, preview)
      }
    })
  })
}
