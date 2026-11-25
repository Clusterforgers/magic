// Goal: crowded tables stay readable at 1400x900 with the side column open: three Delves heroes at ten permanents each draw every creature 76 px wide or more, inside their boards; a six-seat grid keeps every opponent's pod inside its frame with the lands chip and the untapped sources whole and in view, and a row of cards whole on every board; and near 1024 px (1024x768, 1100x800) the card preview keeps to 30 % of the width and covers neither a pod nor the card it shows.
import { expect, test } from './test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SIZE = { width: 1400, height: 900 }

async function open(page: Page, query: string): Promise<void> {
  await page.setViewportSize(SIZE)
  await page.goto(`${DEV_WEB_URL}/dev/table?${query}`)
  await page.waitForFunction(() => document.querySelectorAll('[data-battlefield-seat]').length > 0)
  // The split settles on the measured pod heads within a frame or two.
  await page.waitForTimeout(800)
}

test.describe('crowded tables at 1400x900', () => {
  test('three heroes at ten permanents each draw creatures of 76 px or more inside their boards', async ({
    page,
  }) => {
    await open(page, 'seats=4&boss=0&permanents=10')
    const boards = await page.evaluate(() =>
      [...document.querySelectorAll<HTMLElement>('[data-battlefield-seat]')].map((board) => {
        const box = board.getBoundingClientRect()
        const cards = [...board.querySelectorAll('[data-lane="creatures"] [data-object-id]')]
          .map((card) => card.getBoundingClientRect())
          .filter((card) => card.width > 0)
        return {
          seat: board.getAttribute('data-battlefield-seat'),
          boss: board.closest('[data-layout="boss"]') !== null,
          widest: Math.max(0, ...cards.map((card) => card.width)),
          outside: cards.filter((card) => card.bottom > box.bottom + 1 || card.top < box.top - 1).length,
        }
      }),
    )
    const heroes = boards.filter((board) => !board.boss)
    expect(heroes).toHaveLength(3)
    for (const hero of heroes) {
      expect(hero.widest, `seat ${hero.seat}`).toBeGreaterThanOrEqual(76)
      expect(hero.outside, `seat ${hero.seat}`).toBe(0)
    }
  })

  test("a six-seat grid keeps every opponent's pod and lands whole inside its frame", async ({ page }) => {
    await open(page, 'seats=6&mode=grid')
    const frames = await page.evaluate(() =>
      [...document.querySelectorAll<HTMLElement>('[data-slot="seat-frame"]')].map((frame) => {
        const box = frame.getBoundingClientRect()
        const pod = frame.querySelector('section[data-variant="line"]')?.getBoundingClientRect() ?? null
        const figures = frame.querySelector('[data-slot="pod-figures"]')?.getBoundingClientRect() ?? null
        const whole = (selector: string): boolean => {
          const element = frame.querySelector(selector)
          if (element === null || figures === null) return false
          const part = element.getBoundingClientRect()
          return (
            part.width > 0 &&
            part.left >= figures.left - 0.5 &&
            part.right <= figures.right + 0.5 &&
            part.top >= box.top &&
            part.bottom <= box.bottom
          )
        }
        const board = frame.querySelector('[data-battlefield-seat]')?.getBoundingClientRect() ?? null
        const shown =
          board === null
            ? 0
            : [...frame.querySelectorAll('[data-battlefield-seat] [data-object-id]')]
                .map((card) => card.getBoundingClientRect())
                .filter(
                  (card) =>
                    card.width > 0 && card.top >= board.top - 0.5 && card.bottom <= board.bottom + 0.5,
                ).length
        return {
          seat: frame.querySelector('[data-seat-area]')?.getAttribute('data-seat-area'),
          podInside: pod !== null && pod.bottom <= box.bottom + 0.5,
          lands: whole('[data-slot="land-summary"]'),
          sources: whole('[data-slot="mana-sources"]'),
          shown,
        }
      }),
    )
    expect(frames).toHaveLength(5)
    for (const frame of frames) {
      expect(frame.podInside, `seat ${frame.seat}`).toBe(true)
      expect(frame.lands, `seat ${frame.seat}`).toBe(true)
      expect(frame.sources, `seat ${frame.seat}`).toBe(true)
      expect(frame.shown, `seat ${frame.seat}`).toBeGreaterThan(0)
    }
  })
})

test.describe('the card preview near 1024 px', () => {
  for (const [width, height, seats] of [
    [1024, 768, 2],
    [1100, 800, 4],
  ] as const) {
    test(`at ${width}x${height} with ${seats} seats keeps to its share and covers no pod nor its card`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height })
      await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}`)
      await page.waitForFunction(() => document.querySelectorAll('[data-battlefield-seat]').length > 0)
      await page.waitForTimeout(600)
      const cards = page.locator('[data-battlefield-seat="0"] [data-object-id]')
      const count = await cards.count()
      for (const at of [0, Math.floor(count / 3), Math.floor(count / 2)]) {
        const card = cards.nth(at)
        await card.hover({ force: true })
        const panel = page.getByTestId('card-preview')
        await expect(panel).toBeVisible()
        const shown = must(await card.boundingBox())
        const preview = must(await panel.boundingBox())
        const meets = (a: Rect, b: Rect): boolean =>
          a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
        const pods = await page.evaluate(() =>
          [...document.querySelectorAll('[data-keep-clear="pod"]')].map((pod) => {
            const { x, y, width, height } = pod.getBoundingClientRect()
            return { x, y, width, height }
          }),
        )
        expect(preview.width, `card ${at}`).toBeLessThanOrEqual(width * 0.3 + 1)
        expect(meets(preview, shown), `card ${at}`).toBe(false)
        // A card left of the middle leaves the right side free: no pod lies under the panel.
        if (shown.x + shown.width / 2 < width / 2) {
          expect(
            pods.filter((pod) => meets(preview, pod)),
            `card ${at}`,
          ).toEqual([])
        }
        await page.mouse.move(2, 2)
      }
    })
  }
})

interface Rect {
  x: number
  y: number
  width: number
  height: number
}

const must = <T>(value: T | null): T => {
  if (value === null) throw new Error('not drawn')
  return value
}
