// Goal: on the development table's combat scenes (a block at three seats, teams, Two-Headed Giant, the Delves boss, a planeswalker attacked) at desktop, tablet and phone sizes on either side, the combat forecast is drawn as badges on the cards (what each deals, takes and whether it dies), each with an aria-label and a sign or icon, inside its card and clear of the stat box and the corner marks; the attacked pod or tile says what its life and its planeswalkers would lose; and the declaration's dock or bar shows the total in labelled chips inside the window.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SIZES = [
  { name: 'desktop', width: 1400, height: 900 },
  { name: 'tablet', width: 1024, height: 768 },
  { name: 'phone', width: 390, height: 844 },
  { name: 'phone on its side', width: 844, height: 390 },
] as const

const BLOCKS = [
  { name: 'a block at three seats', query: 'prompt=block' },
  { name: 'a block between teams', query: 'prompt=block&seats=4&teams=2v2' },
  { name: 'a Two-Headed Giant block', query: 'prompt=block&seats=4&teams=2hg' },
  // A phone's boss table shows the heroes' rows folded: the cards in combat may all be out of view, the pods say it.
  { name: 'a block at the Delves boss', query: 'prompt=block&seats=4&boss=0', foldsOnPhone: true },
] as const

const BADGES = '[data-slot="forecast-dealt"], [data-slot="forecast-damage"], [data-slot="forecast-doomed"]'

async function open(page: Page, query: string, width: number, height: number): Promise<void> {
  await page.setViewportSize({ width, height })
  await page.goto(`${DEV_WEB_URL}/dev/table?${query}`)
  await page.waitForFunction(() => document.querySelectorAll('[data-battlefield-seat]').length > 0)
  await page.waitForTimeout(600)
}

interface Box {
  left: number
  top: number
  right: number
  bottom: number
}

/** What each drawn badge says and where it stands against its card, its card's stat box and corner marks. */
async function badges(page: Page) {
  return page.locator(BADGES).evaluateAll((elements) => {
    const box = (element: Element | null) => {
      if (element === null) return null
      const rect = element.getBoundingClientRect()
      return { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom }
    }
    return elements.flatMap((element) => {
      const card = element.closest('button[data-object-id]')
      const rect = element.getBoundingClientRect()
      // A badge in a card scrolled out of its row is not on screen.
      if (card === null || rect.width === 0) return []
      const clip = card.closest('[data-battlefield-seat]')?.getBoundingClientRect()
      if (clip !== undefined && (rect.bottom < clip.top || rect.top > clip.bottom)) return []
      return [
        {
          id: card.getAttribute('data-object-id'),
          slot: element.getAttribute('data-slot'),
          role: element.getAttribute('role'),
          label: element.getAttribute('aria-label') ?? '',
          text: element.textContent ?? '',
          icon: element.querySelector('svg') !== null,
          box: box(element),
          card: box(card),
          stat: box(card.querySelector('[data-slot="card-pt"]')),
          marks: [...card.querySelectorAll('[data-slot="card-note"], [data-slot="cost-now"]')].map(box),
        },
      ]
    })
  })
}

const overlaps = (a: Box, b: Box): boolean =>
  a.left < b.right - 1 && b.left < a.right - 1 && a.top < b.bottom - 1 && b.top < a.bottom - 1

const inside = (a: Box, b: Box): boolean =>
  a.left >= b.left - 1 && a.right <= b.right + 1 && a.top >= b.top - 1 && a.bottom <= b.bottom + 1

for (const size of SIZES) {
  test.describe(`combat forecast badges on the ${size.name}`, () => {
    for (const scene of BLOCKS) {
      test(`${scene.name}: badges on the cards and the total in the dock`, async ({ page }) => {
        const errors: string[] = []
        page.on('pageerror', (error) => errors.push(error.message))
        await open(page, scene.query, size.width, size.height)

        const drawn = await badges(page)
        const folded = 'foldsOnPhone' in scene && size.width < 900
        if (!folded) {
          expect(drawn.length).toBeGreaterThan(0)
          expect(drawn.some((badge) => badge.slot === 'forecast-dealt')).toBe(true)
        }
        for (const badge of drawn) {
          expect(badge.role, `${badge.slot} of ${badge.id}`).toBe('img')
          expect(badge.label, `${badge.slot} of ${badge.id}`).toMatch(
            /^(Deals \d+ damage|Takes \d+ damage|Dies)/,
          )
          // The meaning is in a sign or an icon too, never in the colour alone.
          expect(badge.icon || /^≈?-\d/.test(badge.text), `${badge.slot} of ${badge.id}`).toBe(true)
          if (badge.box === null || badge.card === null) continue
          expect(inside(badge.box, badge.card), `${badge.slot} of ${badge.id} inside its card`).toBe(true)
          if (badge.stat !== null)
            expect(overlaps(badge.box, badge.stat), `${badge.slot} of ${badge.id} over the stat box`).toBe(
              false,
            )
          for (const mark of badge.marks)
            if (mark !== null)
              expect(overlaps(badge.box, mark), `${badge.slot} of ${badge.id} over a corner mark`).toBe(false)
        }

        // The viewer is attacked: their pod or tile says what their life would lose.
        await expect(page.locator('[data-slot="life-forecast"]').first()).toHaveAttribute(
          'aria-label',
          /Life would go from|Takes no combat damage/,
        )

        const total = page.locator('[data-slot="forecast-total"]:visible')
        await expect(total).toHaveCount(1)
        await expect(total).toHaveAttribute('role', 'group')
        const chips = total.locator('[data-slot="total-chip"]')
        await expect(chips.first()).toBeVisible()
        const take = total.locator('[data-slot="total-chip"][data-kind="takes"]')
        await expect(take).toHaveAttribute('aria-label', /^You(r team)? would take \d+ damage/)
        const chipBoxes = await chips.evaluateAll((elements) =>
          elements.map((element) => {
            const rect = element.getBoundingClientRect()
            return { right: rect.right, bottom: rect.bottom, left: rect.left, top: rect.top }
          }),
        )
        for (const chip of chipBoxes) {
          expect(chip.left).toBeGreaterThanOrEqual(0)
          expect(chip.right).toBeLessThanOrEqual(size.width)
          expect(chip.bottom).toBeLessThanOrEqual(size.height)
        }
        expect(errors).toEqual([])
      })
    }

    test('a planeswalker attacked: its badge on the card and its chip on the pod', async ({ page }) => {
      await open(page, 'prompt=forecast-walker', size.width, size.height)
      const chip = page.locator('[data-slot="walker-forecast"]:visible').first()
      await expect(chip).toHaveAttribute('role', 'img')
      await expect(chip).toHaveAttribute(
        'aria-label',
        'Chandra, Torch of Defiance would take 6 damage and die',
      )
      await expect(chip).toContainText('-6')
      const id = await chip.getAttribute('data-object-id')
      const card = page.locator(`button[data-object-id="${id}"]`)
      if ((await card.count()) > 0 && (await card.first().isVisible())) {
        await expect(card.first()).toHaveAttribute('data-doomed', 'true')
        await expect(card.first().locator('[data-slot="forecast-damage"]')).toHaveAttribute(
          'aria-label',
          /^Takes 6 damage/,
        )
      }
    })
  })
}
