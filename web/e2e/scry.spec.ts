// Goal: the Scry prompt of the dev table (`/dev/table?prompt=scry`) is answered from the keyboard alone: a number key sends the card looked at in that place to the bottom pile, Alt with an arrow moves the focused card within its pile, Original order puts everything back, and Confirm with Enter ends the decision.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SCENE = `${DEV_WEB_URL}/dev/table?prompt=scry&turn=you`

/** The cards of a pile, first (nearest the top of the library) to last. */
const pile = (page: Page, name: 'top' | 'bottom') => page.locator(`[data-slot="scry-${name}"] li[data-card]`)

const order = async (page: Page, name: 'top' | 'bottom'): Promise<string[]> =>
  pile(page, name).evaluateAll((items) => items.map((item) => item.getAttribute('data-card') ?? ''))

test('a scry is split, ordered and confirmed with the keyboard', async ({ page }) => {
  await page.goto(SCENE)
  await page.locator('[data-region="controls"]').waitFor()
  await expect(pile(page, 'top')).toHaveCount(4)
  await expect(pile(page, 'bottom')).toHaveCount(0)
  const start = await order(page, 'top')

  // The number key 1 sends the first card looked at to the bottom pile.
  await page.keyboard.press('1')
  await expect(pile(page, 'top')).toHaveCount(3)
  await expect(pile(page, 'bottom')).toHaveCount(1)
  expect(await order(page, 'bottom')).toEqual([start[0]])
  expect(await order(page, 'top')).toEqual(start.slice(1))

  // Alt with an arrow moves the focused card one place toward the bottom of its pile.
  await page.locator(`button[data-card="${start[1]}"][data-move="down"]`).focus()
  await page.keyboard.press('Alt+ArrowRight')
  expect(await order(page, 'top')).toEqual([start[2], start[1], start[3]])

  // Original order puts every card back on top as it was looked at.
  const reset = page.getByRole('button', { name: 'Original order' })
  await expect(reset).toBeEnabled()
  await reset.press('Enter')
  await expect(pile(page, 'bottom')).toHaveCount(0)
  expect(await order(page, 'top')).toEqual(start)

  // Confirm answers the decision and clears it.
  await page.keyboard.press('2')
  const confirm = page.getByRole('button', { name: 'Confirm' })
  await confirm.focus()
  await confirm.press('Enter')
  await expect(confirm).toHaveCount(0)
})
