// Goal: the Delves leaderboard never moves the page's own column: at 1400, 1600 and 1920 px the column's left edge and width are the same with the leaderboard and without it, the leaderboard stands in the right gutter inside the window, and at a phone's width it follows the page's content.
import { expect, test } from './test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

interface Box {
  left: number
  right: number
  top: number
  width: number
}

const box = (page: Page, selector: string): Promise<Box> =>
  page.locator(selector).evaluate((element) => {
    const rect = element.getBoundingClientRect()
    return { left: rect.left, right: rect.right, top: rect.top, width: rect.width }
  })

const open = async (page: Page, width: number) => {
  await page.setViewportSize({ width, height: 900 })
  await page.goto(`${DEV_WEB_URL}/dev/delve?state=list`)
  await expect(page.getByRole('tab', { name: 'Heroic' })).toBeVisible()
}

for (const width of [1400, 1600, 1920]) {
  test(`the leaderboard stands in the right gutter and leaves the column where it is at ${width} px`, async ({
    page,
  }) => {
    await open(page, width)
    const column = await box(page, '[data-slot="page"]')
    const heading = await box(page, 'h1')
    const board = await box(page, '[data-slot="delves-leaderboard"]')
    const inner = await page.evaluate(() => document.documentElement.clientWidth)
    expect(board.left).toBeGreaterThanOrEqual(column.right)
    expect(board.right).toBeLessThanOrEqual(inner)
    expect(board.top).toBeLessThanOrEqual(heading.top + 1)
    await page.locator('[data-slot="delves-leaderboard"]').evaluate((element) => element.remove())
    expect(await box(page, '[data-slot="page"]')).toEqual(column)
    expect(await box(page, 'h1')).toEqual(heading)
    expect(column.width).toBe(768)
    expect(column.left).toBeCloseTo((inner - 768) / 2, 0)
  })
}

test('at a phone the leaderboard follows the page content', async ({ page }) => {
  await open(page, 390)
  const rules = await box(page, 'text=Rules in short')
  const board = await box(page, '[data-slot="delves-leaderboard"]')
  const column = await box(page, '[data-slot="page"]')
  expect(board.top).toBeGreaterThan(rules.top)
  expect(board.left).toBeGreaterThanOrEqual(column.left)
  expect(board.right).toBeLessThanOrEqual(column.right)
})
