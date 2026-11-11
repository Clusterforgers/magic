// Goal: on the development table at a desktop, a tablet and a phone size, the help menu runs the table tour through every step with its card inside the window and a part of the table lit for every step but the first, and opens the rules lexicon, whose search finds a rules term.
import { expect, test } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SIZES = [
  { width: 1400, height: 900 },
  { width: 1024, height: 768 },
  { width: 390, height: 844 },
]

for (const size of SIZES) {
  test(`the tour and the lexicon at ${size.width}x${size.height}`, async ({ page }) => {
    await page.setViewportSize(size)
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=2&turn=you&prompt=priority`)
    await expect(page.locator('[data-region="hand"]')).toBeVisible()

    // The dev route's Effects button lies over the top bar on a phone; the keyboard reaches Help all the same.
    await page.getByRole('button', { name: 'Help' }).focus()
    await page.keyboard.press('Enter')
    await page.getByRole('menuitem', { name: 'Table tour' }).click()
    const card = page.locator('[data-slot="tour-card"]')
    let steps = 0
    const total = Number((await card.getByText(/^Step 1 of \d+$/).textContent())?.split(' ').at(-1))
    expect(total).toBeGreaterThan(5)
    for (;;) {
      await expect(card).toBeVisible()
      steps += 1
      const box = await card.boundingBox()
      expect(box).not.toBeNull()
      if (box !== null) {
        expect(box.x).toBeGreaterThanOrEqual(0)
        expect(box.y).toBeGreaterThanOrEqual(0)
        expect(box.x + box.width).toBeLessThanOrEqual(size.width)
        expect(box.y + box.height).toBeLessThanOrEqual(size.height)
      }
      if (steps > 1) await expect(page.locator('[data-slot="tour-spotlight"]')).toHaveCount(1)
      const done = page.getByRole('button', { name: 'Done' })
      if (await done.isVisible()) {
        await done.click()
        break
      }
      await page.getByRole('button', { name: 'Next' }).click()
    }
    expect(steps).toBe(total)
    await expect(card).toHaveCount(0)

    // The dev route's Effects button lies over the top bar on a phone; the keyboard reaches Help all the same.
    await page.getByRole('button', { name: 'Help' }).focus()
    await page.keyboard.press('Enter')
    await page.getByRole('menuitem', { name: 'Rules lexicon' }).click()
    const lexicon = page.getByRole('dialog', { name: 'Rules lexicon' })
    await lexicon.getByRole('searchbox', { name: 'Search the lexicon' }).fill('upkeep')
    await expect(lexicon.locator('[data-slot="lexicon-entry"]').first()).toContainText('Upkeep')
    await page.keyboard.press('Escape')
    await expect(lexicon).toHaveCount(0)
  })
}
