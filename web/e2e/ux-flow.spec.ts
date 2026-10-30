// Goal: three flow defects of the web client stay fixed on the dev routes (no server): a phone opens a saved deck on its Deck tab with the report, and a wide window shows both panes; the Mana row of the Commands panel starts on a colour of the seat's commander identity; and a seat working in the Commands panel sees the response clock there, in view at 1440x900 and 390x844.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

interface Size {
  width: number
  height: number
}

const WIDE: Size = { width: 1440, height: 900 }
const PHONE: Size = { width: 390, height: 844 }

const label = (size: Size): string => `${size.width}x${size.height}`

/** True when the element's box lies inside the window, so it is seen without scrolling the page. */
async function inView(page: Page, selector: string): Promise<boolean> {
  return page.evaluate((css) => {
    const element = document.querySelector(css)
    if (element === null) return false
    const { left, top, right, bottom } = element.getBoundingClientRect()
    return left >= 0 && top >= 0 && right <= window.innerWidth && bottom <= window.innerHeight
  }, selector)
}

test.describe('the deck builder', () => {
  test('on a phone a saved deck opens on the Deck tab with its report, not on Search', async ({ page }) => {
    await page.setViewportSize(PHONE)
    await page.goto(`${DEV_WEB_URL}/dev/builder?deck=mixed`)
    const show = page.getByRole('group', { name: 'Show' })
    await expect(show.getByRole('button', { name: /^Deck/ })).toHaveAttribute('aria-pressed', 'true')
    await expect(show.getByRole('button', { name: 'Search' })).toHaveAttribute('aria-pressed', 'false')
    await expect(page.getByRole('region', { name: 'Deck', exact: true })).toBeVisible()
    await expect(
      page
        .getByRole('region', { name: 'Deck', exact: true })
        .getByText(/played by hand/)
        .first(),
    ).toBeVisible()
    await expect(page.getByRole('searchbox', { name: 'Search cards' })).toBeHidden()
    await show.getByRole('button', { name: 'Search' }).click()
    await expect(page.getByRole('searchbox', { name: 'Search cards' })).toBeVisible()
    await expect(page.getByRole('region', { name: 'Deck', exact: true })).toBeHidden()
  })

  test('in a wide window the search and the deck stand side by side', async ({ page }) => {
    await page.setViewportSize(WIDE)
    await page.goto(`${DEV_WEB_URL}/dev/builder?deck=mixed`)
    await expect(page.getByRole('region', { name: 'Deck', exact: true })).toBeVisible()
    await expect(page.getByRole('searchbox', { name: 'Search cards' })).toBeVisible()
  })
})

test.describe('the Commands panel', () => {
  test('the Mana row starts on a colour of the seat’s commander identity, not on Green', async ({ page }) => {
    await page.setViewportSize(WIDE)
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=2&panel=mana`)
    const mana = page.locator('select[aria-label="Mana"]')
    await expect(mana).toBeVisible()
    // The fixture seat's commander is Tymna the Weaver, white and black.
    await expect(mana).toHaveValue('White')
  })

  for (const size of [WIDE, PHONE]) {
    test(`the response clock of the seat is in the panel at ${label(size)}`, async ({ page }) => {
      await page.setViewportSize(size)
      await page.goto(`${DEV_WEB_URL}/dev/table?seats=2&prompt=priority-clock&panel=draw`)
      const panel = page.locator('[data-slot="commands-panel"]')
      const clock = panel.locator('[data-slot="commands-clock"]')
      await expect(clock).toBeVisible()
      await expect(clock.getByRole('timer')).toHaveText(/^Passing for you in \d+ s$/)
      expect(await inView(page, '[data-slot="commands-panel"] [data-slot="commands-clock"]')).toBe(true)
      // The clock stays at the top of the panel while its rows scroll.
      await panel.evaluate((element) => element.scrollTo({ top: element.scrollHeight }))
      expect(await inView(page, '[data-slot="commands-panel"] [data-slot="commands-clock"]')).toBe(true)
      const box = await clock.boundingBox()
      const panelBox = await panel.boundingBox()
      expect(box?.y ?? Number.NaN).toBeCloseTo(panelBox?.y ?? Number.NaN, 0)
    })
  }
})
