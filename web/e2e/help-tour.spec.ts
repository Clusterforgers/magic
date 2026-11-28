// Goal: on the development table at a desktop, a tablet and a phone size, with two and four seats, the help menu runs the table tour through every step with its card inside the window and, at every step but the first, one anchor lit with the coach's ring on it; and opens the rules lexicon, whose search finds a rules term.
import { expect, test } from './test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SIZES = [
  { width: 1400, height: 900 },
  { width: 1024, height: 768 },
  { width: 390, height: 844 },
]

/** The lit anchor's element and the ring's own place (not its box: it pulses as it arrives). */
async function lit(page: Page) {
  return page.evaluate(() => {
    const root = document.querySelector<HTMLElement>('[data-slot="table-tour"]')
    const target = root?.dataset['coachLit'] ?? ''
    const zone = /^(hand|board|command|opp|stack):/.exec(target)?.[1]
    const element =
      target === ''
        ? null
        : zone === undefined
          ? document.querySelector(`[data-coach="${target}"]`)
          : document.querySelector(`[data-coach-zone="${zone}"] [data-object-id]`)
    const rect = element?.getBoundingClientRect()
    const rings = [...document.querySelectorAll<HTMLElement>('[data-slot="coach-ring"]')]
    const ring = rings[0]
    return {
      target,
      rings: rings.length,
      element:
        rect === undefined ? null : { x: rect.left, y: rect.top, width: rect.width, height: rect.height },
      ring:
        ring === undefined
          ? null
          : {
              x: parseFloat(ring.style.left),
              y: parseFloat(ring.style.top),
              width: parseFloat(ring.style.width),
            },
    }
  })
}

async function openHelp(page: Page, item: string): Promise<void> {
  // The dev route's Effects button lies over the top bar on a phone; the keyboard reaches Help all the same.
  await page.getByRole('button', { name: 'Help' }).focus()
  await page.keyboard.press('Enter')
  await page.getByRole('menuitem', { name: item }).click()
}

for (const seats of [2, 4])
  for (const size of SIZES) {
    test(`the tour and the lexicon at ${seats} seats, ${size.width}x${size.height}`, async ({ page }) => {
      await page.setViewportSize(size)
      await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}&turn=you&prompt=priority`)
      await expect(page.locator('[data-region="hand"]')).toBeVisible()

      await openHelp(page, 'Table tour')
      const root = page.locator('[data-slot="table-tour"]')
      const card = page.locator('[data-slot="tour-card"]')
      await expect(root).toHaveAttribute('data-tour-index', '0')
      const total = Number(await root.getAttribute('data-tour-count'))
      expect(total).toBeGreaterThan(5)
      for (let step = 0; step < total; step += 1) {
        await expect(root).toHaveAttribute('data-tour-index', String(step))
        await expect(card).toBeVisible()
        if (step > 0) {
          // The light follows the page on the coach's beat: wait for a lit anchor with its ring on it.
          await expect
            .poll(async () => {
              const now = await lit(page)
              if (now.target === '' || now.rings !== 1 || now.element === null || now.ring === null)
                return 'unlit'
              const off =
                Math.abs(now.ring.x + 4 - now.element.x) + Math.abs(now.ring.width - 8 - now.element.width)
              return off < 2 ? 'on' : `off by ${off}`
            })
            .toBe('on')
        }
        const box = await card.boundingBox()
        expect(box).not.toBeNull()
        if (box !== null) {
          expect(box.x).toBeGreaterThanOrEqual(0)
          expect(box.y).toBeGreaterThanOrEqual(0)
          expect(box.x + box.width).toBeLessThanOrEqual(size.width)
          expect(box.y + box.height).toBeLessThanOrEqual(size.height)
        }
        await card.getByRole('button', { name: step === total - 1 ? 'Done' : 'Next' }).click()
      }
      await expect(root).toHaveCount(0)

      await openHelp(page, 'Rules lexicon')
      const lexicon = page.getByRole('dialog', { name: 'Rules lexicon' })
      await lexicon.getByRole('searchbox', { name: 'Search the lexicon' }).fill('upkeep')
      await expect(lexicon.locator('[data-slot="lexicon-entry"]').first()).toContainText('Upkeep')
      await page.keyboard.press('Escape')
      await expect(lexicon).toHaveCount(0)
    })
  }
