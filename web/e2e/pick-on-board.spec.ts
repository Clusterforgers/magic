// Goal: on the development table a target choice in the card overlay with two copies of one creature numbers both on their pictures and on the table, and Pick on board (V) hides the overlay without moving a card, so the second copy is chosen on the table and unchosen again, Confirm stays in reach, and Show choices brings the cards back with the copy chosen; on a desktop window, a phone and a phone on its side.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

type Box = [number, number, number, number]

/** The box of every card of the boards, by object id. */
function boardBoxes(): Record<string, Box> {
  const boxes: Record<string, Box> = {}
  for (const card of document.querySelectorAll<HTMLElement>(
    ':is([data-area="opponents"], [data-area="own"]) button[data-object-id][data-size]',
  )) {
    const { x, y, width, height } = card.getBoundingClientRect()
    boxes[card.dataset['objectId'] ?? ''] = [
      Math.round(x),
      Math.round(y),
      Math.round(width),
      Math.round(height),
    ]
  }
  return boxes
}

/** The boards once two reads in a row agree: the table fits itself over a few frames. */
async function settled(page: Page): Promise<Record<string, Box>> {
  let last = ''
  await expect
    .poll(
      async () => {
        const now = JSON.stringify(await page.evaluate(boardBoxes))
        const same = now === last
        last = now
        return same
      },
      { message: 'the boards settle', intervals: [150] },
    )
    .toBe(true)
  return JSON.parse(last) as Record<string, Box>
}

const SIZES = [
  { name: 'a desktop window', width: 1400, height: 900 },
  { name: 'a phone', width: 390, height: 844 },
  { name: 'a phone on its side', width: 844, height: 390 },
]

for (const { name, width, height } of SIZES) {
  test(`picks the second of two copies on the table on ${name} (${width}x${height})`, async ({ page }) => {
    await page.setViewportSize({ width, height })
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=2&prompt=targets-twins`)
    const overlay = page.locator('[data-slot="choice-overlay"]')
    const toggle = page.locator('[data-slot="pick-on-board"]')
    await expect(toggle).toHaveAccessibleName('Pick on board')

    // The two copies carry their numbers on the pictures; the table carries the same on their cards.
    const twins = overlay.locator('button[data-candidate]:has([data-slot="twin-number"])')
    await expect(twins).toHaveCount(2)
    const second = (await twins.nth(1).getAttribute('data-candidate')) ?? ''
    const number = (await twins.nth(1).locator('[data-slot="twin-number"]').textContent()) ?? ''
    const board = page.locator(
      `:is([data-area="opponents"], [data-area="own"]) button[data-object-id="${second}"][data-size]`,
    )
    expect(await board.evaluate((card) => getComputedStyle(card, '::after').content)).toBe(`"${number}"`)
    const labels = await twins.evaluateAll((cards) => cards.map((card) => card.getAttribute('aria-label')))
    expect(labels[0]).not.toBe(labels[1])

    const before = await settled(page)
    await toggle.click()
    await expect(overlay).toBeHidden()
    await expect(toggle).toHaveAccessibleName('Show choices')
    const after = await settled(page)
    expect(after, 'hiding the choices moves no card of the boards').toEqual(before)

    const confirm = page.getByRole('button', { name: 'Confirm targets' })
    await expect(confirm).toBeDisabled()
    await expect(board).toHaveAttribute('data-highlight', 'legal')
    await board.click()
    await expect(board).toHaveAttribute('data-highlight', 'selected')
    await expect(confirm).toBeEnabled()
    await expect(confirm).toBeInViewport()
    await board.click()
    await expect(board).toHaveAttribute('data-highlight', 'legal')
    await board.click()

    await page.keyboard.press('v')
    await expect(overlay).toBeVisible()
    await expect(overlay.locator(`button[data-candidate="${second}"]`)).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    await expect(twins.nth(0)).toHaveAttribute('aria-pressed', 'false')
    expect(await settled(page), 'showing the choices again moves no card').toEqual(before)
  })
}
