// Goal: on the dev table a cost pick that takes the ability's target (`?prompt=sacrifice-targeted`) and a target pick of what the cost already takes (`?prompt=targets-paid`) badge that card, warn before the answer is sent with the warning and both answers inside the window, return to the choice from the keyboard (Escape) with nothing sent, and send the answer on "Pay anyway" or "Target anyway"; on the duel board, at four seats, with the side column folded and on a phone.
import { expect, test } from './test'
import type { Locator, Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SCENES = [
  { name: 'duel', query: 'board=duel', size: { width: 1440, height: 900 } },
  { name: '4 seats', query: 'seats=4', size: { width: 1366, height: 768 } },
  { name: '4 seats, side folded', query: 'seats=4&side=folded', size: { width: 1366, height: 768 } },
  { name: 'phone', query: 'board=duel', size: { width: 390, height: 844 } },
] as const

/** Whether `locator`'s box lies wholly inside the window. */
async function inside(page: Page, locator: Locator): Promise<boolean> {
  const box = await locator.boundingBox()
  const view = page.viewportSize()
  if (box === null || view === null) return false
  return (
    box.x >= 0 &&
    box.y >= 0 &&
    box.x + box.width <= view.width + 0.5 &&
    box.y + box.height <= view.height + 0.5
  )
}

/** The warning is up: its words, the two answers in the window, the safe one focused. */
async function warned(page: Page, anyway: string): Promise<void> {
  const warning = page.locator('[data-slot="cost-target-warning"]')
  await expect(warning).toContainText('pays the cost. It will have no target.')
  const back = page.getByRole('button', { name: /Choose another/ })
  const send = page.getByRole('button', { name: anyway })
  await expect(back).toBeFocused()
  expect(await inside(page, back)).toBe(true)
  expect(await inside(page, send)).toBe(true)
  await expect(page.locator('[data-slot="undo"]')).toHaveCount(0)
}

for (const scene of SCENES) {
  test(`a sacrifice of the ability's target is warned of and paid anyway (${scene.name})`, async ({
    page,
  }) => {
    await page.setViewportSize(scene.size)
    await page.goto(`${DEV_WEB_URL}/dev/table?${scene.query}&prompt=sacrifice-targeted`)
    const badge = page.locator('[data-candidate] [data-slot="card-badge"]')
    await expect(badge).toHaveText('Targeted')
    // The keyboard alone: 1 picks the first card (the target), Enter on Confirm commits.
    await page.keyboard.press('1')
    const confirm = page.getByRole('button', { name: 'Confirm', exact: true })
    await confirm.focus()
    await page.keyboard.press('Enter')
    await warned(page, 'Pay anyway')
    await page.keyboard.press('Escape')
    await expect(page.locator('[data-slot="cost-target-warning"]')).toHaveCount(0)
    await expect(confirm).toBeVisible()
    await confirm.click()
    await warned(page, 'Pay anyway')
    await page.getByRole('button', { name: 'Pay anyway' }).click()
    await expect(page.locator('[data-decision="ChooseCards"]')).toHaveCount(0)
  })

  test(`a target the cost takes is warned of and chosen anyway (${scene.name})`, async ({ page }) => {
    await page.setViewportSize(scene.size)
    await page.goto(`${DEV_WEB_URL}/dev/table?${scene.query}&prompt=targets-paid`)
    const paid = page.getByRole('button', { name: /, Pays the cost$/ })
    await expect(paid).toHaveCount(1)
    await paid.click()
    await warned(page, 'Target anyway')
    await page.keyboard.press('Escape')
    await expect(page.locator('[data-slot="cost-target-warning"]')).toHaveCount(0)
    await paid.click()
    await warned(page, 'Target anyway')
    await page.getByRole('button', { name: 'Target anyway' }).click()
    await expect(page.locator('[data-decision="ChooseTargets"]')).toHaveCount(0)
  })
}
