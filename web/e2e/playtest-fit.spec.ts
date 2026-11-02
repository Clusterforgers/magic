// Goal: the overlaps the playtest screenshots showed stay fixed at the sizes of the design: a card's type line ends above the mana symbols at its foot, the crown and tax of a commander in the hand lie clear of its power and toughness, a compact tapped land shows its Tapped chip whole on a phone, a pod of the four-seat table keeps its zone chips inside it, the lobby's Ready bar stays inside the deck column, and a phone's turn badge shows whose turn it is uncut.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

interface Size {
  width: number
  height: number
}

const SIZES: readonly Size[] = [
  { width: 1920, height: 1080 },
  { width: 1440, height: 900 },
  { width: 1366, height: 768 },
  { width: 1280, height: 720 },
  { width: 1024, height: 768 },
  { width: 390, height: 844 },
]

const label = (size: Size): string => `${size.width}x${size.height}`

async function atSize(page: Page, size: Size): Promise<void> {
  await page.setViewportSize(size)
  await page.waitForTimeout(250)
}

test.describe('the duel table', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(`${DEV_WEB_URL}/dev/table?board=duel`)
    await page.waitForSelector('[data-slot="mana-strip"]')
  })

  test('a type line ends above the mana symbols of its card', async ({ page }) => {
    for (const size of SIZES) {
      await atSize(page, size)
      const clashes = await page.evaluate(() =>
        Array.from(document.querySelectorAll<HTMLElement>('[data-slot="mana-strip"]')).flatMap((strip) => {
          const line = strip.parentElement?.querySelector<HTMLElement>('[data-slot="card-type-line"]')
          if (line === null || line === undefined) return []
          const top = strip.getBoundingClientRect().top
          const bottom = line.getBoundingClientRect().bottom
          return bottom > top + 0.5 ? [`${line.textContent ?? ''}: ${bottom} > ${top}`] : []
        }),
      )
      expect(clashes, label(size)).toEqual([])
    }
  })

  test('the tax of a commander in the hand lies clear of its power and toughness', async ({ page }) => {
    for (const size of SIZES) {
      await atSize(page, size)
      const clashes = await page.evaluate(() =>
        Array.from(document.querySelectorAll<HTMLElement>('[data-slot="hand-commander-tax"]')).flatMap(
          (pill) => {
            const stat =
              pill.parentElement?.querySelector<HTMLElement>('[data-slot="card-face"]')?.lastElementChild
            if (stat === null || stat === undefined) return []
            const a = pill.getBoundingClientRect()
            const b = stat.getBoundingClientRect()
            const overlap = a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
            return overlap ? [`${stat.textContent ?? ''}`] : []
          },
        ),
      )
      expect(clashes, label(size)).toEqual([])
    }
  })

  test('a phone shows the whole of whose turn it is', async ({ page }) => {
    await atSize(page, { width: 390, height: 844 })
    // The words up to the turn number ("Bo's turn · 5") end inside the pill; the step after them may be cut.
    const room = await page.evaluate(() => {
      const pill = document.querySelector<HTMLElement>('[data-slot="banner"] > *')
      const text = pill === null ? null : document.createTreeWalker(pill, NodeFilter.SHOW_TEXT).nextNode()
      if (pill === null || text === null || text.textContent === null) return null
      const words = text.textContent
      const second = words.indexOf('·', words.indexOf('·') + 1)
      const range = document.createRange()
      range.setStart(text, 0)
      range.setEnd(text, second === -1 ? words.length : second)
      return {
        words,
        end: range.getBoundingClientRect().right,
        edge: pill.getBoundingClientRect().right - 12,
      }
    })
    expect(room).not.toBeNull()
    expect(room?.words ?? '').toContain('turn')
    expect(room?.end ?? Infinity).toBeLessThanOrEqual(room?.edge ?? 0)
  })
})

test('the Ready bar stays inside the deck column of the lobby', async ({ page }) => {
  await page.goto(`${DEV_WEB_URL}/dev/lobby`)
  await page.waitForSelector('[data-slot="ready-bar"]')
  for (const size of SIZES.filter((each) => each.width >= 1000)) {
    await atSize(page, size)
    const sides = await page.evaluate(() => {
      const bar = document.querySelector('[data-slot="ready-bar"]')?.getBoundingClientRect()
      const column = document.querySelector('[data-slot="lobby-deck"]')?.getBoundingClientRect()
      if (bar === undefined || column === undefined) return null
      return { left: bar.left - column.left, right: column.right - bar.right }
    })
    expect(sides, label(size)).not.toBeNull()
    expect(sides?.left ?? -1, label(size)).toBeGreaterThanOrEqual(-0.5)
    expect(sides?.right ?? -1, label(size)).toBeGreaterThanOrEqual(-0.5)
  }
})

test('a compact tapped land on a phone shows its Tapped chip whole', async ({ page }) => {
  await page.goto(`${DEV_WEB_URL}/dev/table?board=duel`)
  await page.waitForSelector('[data-slot="mana-strip"]')
  await atSize(page, { width: 390, height: 844 })
  const cut = await page.evaluate(() => {
    const chips = [
      ...document.querySelectorAll<HTMLElement>(
        '[data-slot="card-face"][data-turned] [data-slot="card-status"]',
      ),
    ]
    return chips.filter((chip) => {
      const art = chip.closest<HTMLElement>('[data-slot="card-art"]')
      if (art === null) return false
      const mine = chip.getBoundingClientRect()
      const room = art.getBoundingClientRect()
      return mine.bottom > room.bottom + 0.5 || mine.right > room.right + 0.5
    }).length
  })
  expect(cut).toBe(0)
})

test('a seat pod keeps its zone chips inside the pod on the four-seat table', async ({ page }) => {
  await page.goto(`${DEV_WEB_URL}/dev/table?seats=4`)
  await page.waitForSelector('[data-seat][data-you]')
  for (const size of [
    { width: 1440, height: 900 },
    { width: 1920, height: 1080 },
  ]) {
    await atSize(page, size)
    const outside = await page.evaluate(() =>
      [...document.querySelectorAll<HTMLElement>('[data-seat]')].flatMap((pod) => {
        const box = pod.getBoundingClientRect()
        return [...pod.querySelectorAll<HTMLElement>('[data-zone]')]
          .filter((zone) => {
            const at = zone.getBoundingClientRect()
            return at.width > 0 && (at.bottom > box.bottom + 0.5 || at.right > box.right + 0.5)
          })
          .map(() => pod.getAttribute('data-seat'))
      }),
    )
    expect(outside, label(size)).toEqual([])
  }
})
