// Goal: on the development table's boss fight (`?prompt=priority-blocked`) a press on a hand card the offer leaves out says why in one line at the card, inside the window, at 1400x900 and on a phone: the One Voice modifier (its boss chip lit where the row shows it), sorcery speed and too little mana; before a press nothing is said over the hand.
import { expect, test } from './test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SIZES = [
  { name: 'desktop', width: 1400, height: 900 },
  { name: 'phone', width: 390, height: 844 },
] as const

async function open(page: Page, width: number, height: number): Promise<void> {
  await page.setViewportSize({ width, height })
  await page.goto(`${DEV_WEB_URL}/dev/table?seats=4&boss=0&prompt=priority-blocked`)
  await page.waitForFunction(() => document.querySelectorAll('[data-battlefield-seat]').length > 0)
  await page.waitForTimeout(600)
}

/** Presses the dimmed card whose spoken name holds `words` with the keyboard, as a tap or a click does. */
async function press(page: Page, words: string): Promise<void> {
  const card = page.locator(`button[data-object-id][data-highlight="dimmed"][aria-label*="${words}"]`).first()
  await card.focus()
  await page.keyboard.press('Enter')
}

const tell = (page: Page) => page.locator('[data-slot="blocked-tell"]')

for (const size of SIZES) {
  test.describe(`a blocked hand card on the ${size.name}`, () => {
    test('says nothing before a press, then its reason in one line inside the window', async ({ page }) => {
      await open(page, size.width, size.height)
      await expect(tell(page)).toHaveCount(0)
      const cases: [string, RegExp][] = [
        [
          'One Voice: you have cast your spell this turn',
          /^One Voice: you have cast your spell this turn\.$/,
        ],
        [
          'Sorcery speed: only in your main phase',
          /^Sorcery speed: only in your main phase with an empty stack\.$/,
        ],
        ['you have 1 mana available', /^Needs .+: you have 1 mana available\.$/],
      ]
      for (const [words, line] of cases) {
        await press(page, words)
        await expect(tell(page)).toHaveText(line)
        const box = await tell(page).boundingBox()
        expect(box).not.toBeNull()
        if (box !== null) {
          expect(box.x).toBeGreaterThanOrEqual(0)
          expect(box.x + box.width).toBeLessThanOrEqual(size.width)
          expect(box.y).toBeGreaterThanOrEqual(0)
          expect(box.y + box.height).toBeLessThanOrEqual(size.height)
        }
        if (words.startsWith('One Voice')) {
          const chip = page.locator('[data-slot="boss-ability"]', { hasText: 'One Voice' })
          if (await chip.isVisible()) await expect(chip).toHaveAttribute('data-blocking', 'true')
          const shots = process.env.MTG_SHOT_DIR
          if (shots !== undefined) {
            // The line follows a rising hand card once it has settled.
            await page.waitForTimeout(400)
            await page.screenshot({ path: `${shots}/blocked-${size.name}.png` })
          }
        }
        await page.keyboard.press('Escape')
        await expect(tell(page)).toHaveCount(0)
      }
    })
  })
}
