// Goal: in the deck builder a line's "Play by hand" toggle marks a Verified card, the live check then shows it as played by hand with the owner's reason, and the toggle gives it back to the rules, at a wide and a phone window, on the dev route (no server).
import { expect, test } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SIZES = [
  { width: 1400, height: 900 },
  { width: 390, height: 844 },
]

for (const size of SIZES) {
  test(`a line's toggle plays a Verified card by hand and gives it back at ${size.width}x${size.height}`, async ({
    page,
  }) => {
    await page.setViewportSize(size)
    await page.goto(`${DEV_WEB_URL}/dev/builder?deck=verified`)
    const deck = page.getByRole('region', { name: 'Deck' })
    const line = deck.locator('li[data-line^="main:"]').filter({ hasText: 'Mountain' })
    const toggle = line.getByRole('button', { name: 'Play Mountain by hand' })
    await expect(toggle).toHaveAttribute('aria-pressed', 'false')
    await expect(line).toHaveAttribute('data-support', 'verified')

    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-pressed', 'true')
    await expect(line).toHaveAttribute('data-support', 'manual')
    await expect(line.locator('[data-slot="line-check"]')).toContainText(/owner chose to play it by hand/i)
    await expect(deck.getByText('Unsaved changes')).toBeVisible()
    const box = await toggle.boundingBox()
    expect(box?.width ?? 0).toBeGreaterThanOrEqual(32)

    await toggle.click()
    await expect(toggle).toHaveAttribute('aria-pressed', 'false')
    await expect(line).toHaveAttribute('data-support', 'verified')
  })
}
