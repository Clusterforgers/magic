// Goal: on the development table a land the viewer taps for mana at priority is untapped again by a second press, its mana gone from the pool again, in a duel and at four seats, at 1400x900 and on a phone (390x844, in the sheet of the viewer's lands).
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

/** The viewer's first basic land on the board that is offered for mana. */
async function offeredLand(page: Page) {
  const land = page
    .locator(`section[aria-label$="'s battlefield"] button[data-highlight="legal"][aria-label*="Basic Land"]`)
    .first()
  const id = await land.getAttribute('data-object-id')
  return page.locator(`button[data-object-id="${id}"]`)
}

for (const [width, height] of [
  [1400, 900],
  [390, 844],
] as const) {
  for (const seats of [2, 4]) {
    test.describe(`undoing a tap for mana at ${seats} seats, ${width}x${height}`, () => {
      test.use({ viewport: { width, height } })

      test('a land tapped for mana untaps on a second press and its mana leaves the pool', async ({
        page,
      }) => {
        await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}&turn=you&prompt=priority-main`)
        // A phone folds the viewer's lands into a chip; they are pressed in the sheet it opens.
        if (width < 600) await page.getByRole('button', { name: /^Show Alice's lands/ }).click()
        const land = await offeredLand(page)
        // The fixture's pools already float some mana; the press adds to it and the second press takes it back.
        const pools = async (): Promise<string> =>
          (await page.getByRole('group', { name: 'Mana pool' }).allTextContents()).join('|')
        const before = await pools()
        await land.click({ position: { x: 10, y: 6 } })
        await expect(land).toHaveAttribute('data-tapped', 'true')
        await expect.poll(pools).not.toBe(before)
        await expect(land).toHaveAccessibleName(/press to untap .* and take its mana back/)
        await land.click({ position: { x: 10, y: 6 } })
        await expect(land).not.toHaveAttribute('data-tapped')
        await expect.poll(pools).toBe(before)
      })
    })
  }
}
