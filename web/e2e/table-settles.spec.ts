// Goal: the development table settles after its first render at 5, 6, 7 and 8 seats in grid mode at 1280x720, 1920x1080 and 2180x1290: the sizes of the boards and lanes change at most twice and then hold, and every board of the opponents' grid draws one card size.
import { expect, test } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

/** The most changes of the boards' sizes after the first frame with boards: a measure, then the split's answer. */
const MOST_CHANGES = 3

const SIZES = [
  [1280, 720],
  [1920, 1080],
  [2180, 1290],
] as const

for (const seats of [5, 6, 7, 8]) {
  for (const [width, height] of SIZES) {
    test(`a ${seats}-seat grid settles at ${width}x${height}`, async ({ page }) => {
      await page.setViewportSize({ width, height })
      await page.addInitScript(() => {
        const changes: number[] = []
        Object.assign(window, { __layoutChanges: changes })
        let last = ''
        const look = (): void => {
          const boxes = [
            ...document.querySelectorAll<HTMLElement>('[data-slot="battlefield-zone"], [data-lane]'),
          ]
          const now = boxes.map((box) => `${box.clientWidth}x${box.clientHeight}`).join('|')
          if (boxes.length > 0 && now !== last) {
            changes.push(performance.now())
            last = now
          }
          requestAnimationFrame(look)
        }
        requestAnimationFrame(look)
      })
      await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}&mode=grid`)
      await page.locator('[data-battlefield-seat]').first().waitFor({ state: 'attached' })
      await page.waitForTimeout(1500)
      const settled = await page.evaluate(
        () => (window as unknown as { __layoutChanges: number[] }).__layoutChanges.length,
      )
      expect(settled).toBeLessThanOrEqual(MOST_CHANGES)
      await page.waitForTimeout(1000)
      const later = await page.evaluate(
        () => (window as unknown as { __layoutChanges: number[] }).__layoutChanges.length,
      )
      expect(later).toBe(settled)
      const cards = await page.evaluate(() =>
        [...document.querySelectorAll('[data-layout="grid"] [data-lane="creatures"]')].map((lane) =>
          getComputedStyle(lane.firstElementChild ?? lane).getPropertyValue('--card-w'),
        ),
      )
      expect(new Set(cards).size).toBeLessThanOrEqual(1)
    })
  }
}
