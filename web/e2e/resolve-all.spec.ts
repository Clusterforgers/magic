// Goal: over a stack of two or more objects "Resolve all" stands beside the primary button in every table mode (duel, grid of 2 to 6 seats, teams, Two-Headed Giant, a boss fight) at laptop and desktop sizes, inside the window and uncovered, the primary button still last and Space and Enter unchanged; a press starts the yield and the prompt gives way to the passing line; a phone holds it in the menu of other ways to pass.
import { expect, test } from '@playwright/test'
import type { Locator, Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const CONTROLS = '[data-region="controls"]'
const resolveAll = (page: Page): Locator => page.locator(`${CONTROLS} [data-slot="resolve-all"]`)
const primary = (page: Page): Locator => page.locator(`${CONTROLS} [data-slot="primary-action"]`)

/** The tables of every mode, each with the fixture's three spells on the stack and the viewer holding priority. */
const TABLES = [
  ['the duel board', 'board=duel&prompt=priority'],
  ['2 seats', 'seats=2&prompt=priority'],
  ['4 seats', 'seats=4&prompt=priority&turn=you'],
  ['6 seats', 'seats=6&prompt=priority'],
  ['two teams of two', 'teams=2v2&prompt=priority&turn=you'],
  ['Two-Headed Giant', 'teams=2hg&prompt=priority&turn=you'],
  ['a boss fight', 'boss=3&prompt=priority&turn=you'],
] as const

const SIZES = [
  { width: 1280, height: 720 },
  { width: 1366, height: 768 },
  { width: 1920, height: 1080 },
] as const

/** Whether the centre of `locator` is inside the window and the element there is `locator` itself or inside it. */
async function pressable(locator: Locator): Promise<boolean> {
  return locator.evaluate((element) => {
    const box = element.getBoundingClientRect()
    const x = box.left + box.width / 2
    const y = box.top + box.height / 2
    if (x < 0 || y < 0 || x > window.innerWidth || y > window.innerHeight) return false
    const hit = document.elementFromPoint(x, y)
    return hit !== null && element.contains(hit)
  })
}

for (const size of SIZES) {
  for (const [name, query] of TABLES) {
    test(`${name} at ${size.width}x${size.height}: "Resolve all" beside the primary button`, async ({
      page,
    }) => {
      await page.setViewportSize(size)
      await page.goto(`${DEV_WEB_URL}/dev/table?${query}`)
      await expect(primary(page)).toBeVisible()
      await expect(resolveAll(page)).toBeVisible()
      await expect(resolveAll(page)).toHaveText('Resolve all')
      expect(await pressable(resolveAll(page)), 'Resolve all is in the window and uncovered').toBe(true)
      expect(await pressable(primary(page)), 'the primary button is in the window and uncovered').toBe(true)
      // The primary button keeps the corner: "Resolve all" is the control just before it.
      const next = await resolveAll(page).evaluate((element) =>
        element.nextElementSibling?.getAttribute('data-slot'),
      )
      expect(next).toBe('primary-action')
      await expect(page.locator(`${CONTROLS} [data-slot="secondary-action"]`)).toHaveCount(0)

      await resolveAll(page).click()
      await expect(page.locator('[data-slot="yield-indicator"]')).toHaveAttribute('data-yield', 'Resolve')
      await expect(page.getByText('Passing until the stack is empty')).toBeVisible()
      await expect(primary(page)).toHaveCount(0)
      await expect(resolveAll(page)).toHaveCount(0)
    })
  }
}

test('a stack of one draws "End turn" beside the primary button, not "Resolve all"', async ({ page }) => {
  await page.setViewportSize({ width: 1366, height: 768 })
  await page.goto(`${DEV_WEB_URL}/dev/table?seats=2&prompt=priority-wait`)
  await expect(primary(page)).toBeVisible()
  await expect(page.locator(`${CONTROLS} [data-slot="secondary-action"]`)).toHaveText(/End turn/)
  await expect(resolveAll(page)).toHaveCount(0)
})

test('Enter still ends the turn over a stack of three', async ({ page }) => {
  await page.setViewportSize({ width: 1366, height: 768 })
  await page.goto(`${DEV_WEB_URL}/dev/table?seats=2&prompt=priority`)
  await expect(resolveAll(page)).toBeVisible()
  await page.keyboard.press('Enter')
  await expect(page.locator('[data-slot="yield-indicator"]')).toHaveAttribute('data-yield', 'EndOfTurn')
})

test('a phone holds "Resolve all" in the menu of other ways to pass', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto(`${DEV_WEB_URL}/dev/table?seats=4&prompt=priority&turn=you`)
  await expect(primary(page)).toBeVisible()
  await expect(resolveAll(page)).toHaveCount(0)
  await page.locator(`${CONTROLS} [data-slot="more-passing"]`).click()
  await page.getByRole('menuitem', { name: 'Resolve all' }).click()
  await expect(page.locator('[data-slot="yield-indicator"]')).toHaveAttribute('data-yield', 'Resolve')
})
