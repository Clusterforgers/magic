// Goal: on the review screen with 55 cards to read, one named far wider than the list (`/dev/read?view=long`), at 1280, 1920 and 2560 px wide, the list stays inside its column and cuts the long name short, the loaded card picture stays inside the card panel beside the list, and nothing covers the "55 left" counter.
import { expect, test } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

// A 5:7 PNG of one colour stands in for every card picture, so the picture loads with no server.
const PICTURE = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAUAAAAHCAIAAABPxRC5AAAAEUlEQVR4nGNoMDBARgw05gMAvroeoRUw9eQAAAAASUVORK5CYII=',
  'base64',
)

interface Box {
  left: number
  top: number
  right: number
  bottom: number
}

const overlaps = (a: Box, b: Box): boolean =>
  a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom

for (const width of [1280, 1920, 2560]) {
  test(`the review list and the card picture keep apart at ${width} px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 1200 })
    await page.route('**/api/images/**', (route) =>
      route.fulfill({ body: PICTURE, contentType: 'image/png' }),
    )
    await page.goto(`${DEV_WEB_URL}/dev/read?view=long`)
    await expect(page.locator('[data-slot="review-picture"] [data-status="loaded"]')).toBeVisible()
    await expect(page.getByRole('region', { name: /^To read/ })).toContainText('55 left')

    const look = await page.evaluate(() => {
      const box = (element: Element | null): Box | null => {
        if (element === null) return null
        const { left, top, right, bottom } = element.getBoundingClientRect()
        return { left, top, right, bottom }
      }
      const nav = document.querySelector('nav[aria-label="Cards to review"]')
      const column = nav?.parentElement ?? null
      const counter = document.querySelector('#review-part-read span')
      const at = counter?.getBoundingClientRect()
      const hit =
        at === undefined ? null : document.elementFromPoint(at.left + at.width / 2, at.top + at.height / 2)
      const rows = [...(nav?.querySelectorAll('button[data-item]') ?? [])]
      return {
        nav: box(nav),
        column: box(column),
        widest: Math.max(...rows.map((row) => row.scrollWidth - row.clientWidth)),
        picture: box(document.querySelector('[data-slot="review-picture"]')),
        panel: box(document.querySelector('article')),
        counterFree: hit !== null && counter !== null && (hit === counter || counter.contains(hit)),
      }
    })
    if (look.nav === null || look.column === null || look.picture === null || look.panel === null) {
      throw new Error('the review screen is missing its list, picture or card')
    }
    expect(look.nav.right).toBeLessThanOrEqual(look.column.right + 0.5)
    expect(look.widest).toBeLessThanOrEqual(0)
    expect(look.picture.left).toBeGreaterThanOrEqual(look.panel.left - 0.5)
    expect(look.picture.right).toBeLessThanOrEqual(look.panel.right + 0.5)
    expect(look.picture.left).toBeGreaterThanOrEqual(look.nav.right)
    expect(overlaps(look.picture, look.nav)).toBe(false)
    expect(look.counterFree).toBe(true)
  })
}
