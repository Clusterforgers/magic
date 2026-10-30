// Goal: on the development table at 1920x1080, 1600x1000, 1400x900 and 1024x768 a hand that the dock holds (7 to 16 cards, by what the size allows) shows the name and the mana cost of every card with nothing lying over either, in one row or in rows that lie over each other, and the line does not scroll sideways; a hand that is too large for the size scrolls in one row of whole cards and the first card of the front row shows its name and cost.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

interface Case {
  width: number
  height: number
  /** Hands the dock holds without a scroll. */
  holds: number[]
  /** Hands that scroll, in one row. */
  scrolls: number[]
}

const CASES: Case[] = [
  { width: 1920, height: 1080, holds: [7, 9, 14, 16], scrolls: [] },
  { width: 1600, height: 1000, holds: [7, 9, 14, 16], scrolls: [24] },
  { width: 1400, height: 900, holds: [7, 8], scrolls: [14] },
  { width: 1024, height: 768, holds: [7], scrolls: [12] },
]

/**
 * The list of the hand itself. The dock also holds the commanders of the command zone, in a list of the same slot
 * ("Ada's commanders in the command zone"), so the hand is picked by its list's name.
 */
const HAND = '[data-region="hand"] ul[aria-label$="\'s hand"]'

/** What the page found: how many cards it looked at, and the name and cost lines that something lies over. */
interface Found {
  cards: number
  rows: number
  scrolls: boolean
  covered: string[]
}

/** Opens the development table with `count` cards in the hand, once the dock has laid them out. */
async function open(page: Page, count: number): Promise<void> {
  await page.goto(`${DEV_WEB_URL}/dev/table?seats=4&turn=you&prompt=priority&hand=${count}`)
  await expect(page.locator(`${HAND} button[data-object-id]`)).toHaveCount(count)
  await page.waitForFunction(
    () =>
      document.querySelector<HTMLElement>('[data-slot="hand-fan"]')?.style.getPropertyValue('--card-w') !==
      '',
  )
}

/** Looks at the cards of the hand:for each, the name line and the cost row, which no other element may cover. */
async function look(page: Page, only: 'all' | 'front-start'): Promise<Found> {
  return page.evaluate(
    ({ which, hand }) => {
      const fan = document.querySelector<HTMLElement>(hand)
      const covered: string[] = []
      const cards = [
        ...document.querySelectorAll<HTMLElement>(`${hand} [data-slot="hand-slot"] button[data-object-id]`),
      ]
      const chosen =
        which === 'all'
          ? cards
          : cards
              .filter((card) => card.closest('[data-slot="hand-slot"]')?.hasAttribute('data-back') !== true)
              .slice(0, 1)
      for (const card of chosen) {
        const face = card.querySelector('[data-slot="card-face"]')
        const parts = [
          ['name', face?.firstElementChild ?? null],
          ['cost', card.querySelector('[role="img"][aria-label^="Mana cost"]')],
        ] as const
        for (const [what, element] of parts) {
          if (element === null) continue
          const box = element.getBoundingClientRect()
          // The four corners (a little inside) and the middle of the part must hit the card itself.
          const points = [
            [box.left + 2, box.top + 2],
            [box.right - 2, box.top + 2],
            [box.left + 2, box.bottom - 2],
            [box.right - 2, box.bottom - 2],
            [box.left + box.width / 2, box.top + box.height / 2],
          ] as const
          const hidden = points.some(([x, y]) => {
            const hit = document.elementFromPoint(x, y)
            return hit === null || !card.contains(hit)
          })
          if (hidden) covered.push(`${card.getAttribute('aria-label') ?? card.textContent ?? '?'}: ${what}`)
        }
      }
      return {
        cards: chosen.length,
        rows: Number(fan?.getAttribute('data-rows') ?? 0),
        scrolls: fan !== null && fan.scrollWidth > fan.clientWidth + 1,
        covered,
      }
    },
    { which: only, hand: HAND },
  )
}

for (const { width, height, holds, scrolls } of CASES) {
  test.describe(`the hand at ${width}x${height}`, () => {
    test.use({ viewport: { width, height } })

    for (const count of holds) {
      test(`shows the name and cost of all ${count} cards and does not scroll`, async ({ page }) => {
        await open(page, count)
        const found = await look(page, 'all')
        expect(found.cards).toBe(count)
        expect(found.covered).toEqual([])
        expect(found.scrolls).toBe(false)
      })
    }

    for (const count of scrolls) {
      test(`scrolls ${count} cards in one row, and the first card of the front row shows its name and cost`, async ({
        page,
      }) => {
        await open(page, count)
        const found = await look(page, 'front-start')
        expect(found).toMatchObject({ cards: 1, rows: 1, scrolls: true, covered: [] })
      })
    }
  })
}
