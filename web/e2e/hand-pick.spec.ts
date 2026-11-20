// Goal: a decision that picks among the hand (bottoming after a mulligan, a discard) beside a Satchel of twelve cards keeps every one of twelve cards on offer reachable, in a duel, at four seats and in a boss fight, at 1644x947 and on a phone (390x844): where the dock has the room every card shows whole without scrolling, the Satchel giving way to one card; where it does not, a turn of the wheel over the line moves it sideways, its scrollbar takes the pointer, and the last card can be scrolled to and chosen.
import { expect, test } from '@playwright/test'
import type { Locator, Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const TABLES = [
  { name: 'a duel', query: 'board=duel' },
  { name: 'four seats', query: 'seats=4' },
  { name: 'a boss fight', query: 'seats=2&boss=0&bare=1' },
] as const

const handFan = (page: Page): Locator => page.locator('[data-region="hand"] ul[aria-label$="\'s hand"]')
const handCards = (page: Page): Locator => handFan(page).locator('button[data-object-id]')
const satchelFan = (page: Page): Locator => page.locator('[data-slot="hand-command"] [data-slot="hand-fan"]')

/** The line's scroll offset and how far it can scroll, in px. */
const scrollOf = (line: Locator) =>
  line.evaluate((element) => ({ left: element.scrollLeft, room: element.scrollWidth - element.clientWidth }))

/** Each card's box inside the line's visible box and the window, whole: the cards that show without scrolling. */
const wholeCards = (page: Page) =>
  handFan(page).evaluate((line) => {
    const box = line.getBoundingClientRect()
    return [...line.querySelectorAll('button[data-object-id]')].filter((card) => {
      const rect = card.getBoundingClientRect()
      return (
        rect.left >= box.left - 1 &&
        rect.right <= box.right + 1 &&
        rect.left >= 0 &&
        rect.right <= window.innerWidth + 1 &&
        rect.bottom <= window.innerHeight + 1
      )
    }).length
  })

/** Turns the wheel down over `line` until it stops moving; the offsets it went through. */
async function wheelToEnd(page: Page, line: Locator): Promise<number[]> {
  const box = await line.boundingBox()
  if (box === null) throw new Error('the line is not drawn')
  await page.mouse.move(box.x + box.width / 2, box.y + box.height - 20)
  const offsets = [(await scrollOf(line)).left]
  for (let turn = 0; turn < 40; turn += 1) {
    await page.mouse.wheel(0, 200)
    await expect.poll(async () => (await scrollOf(line)).left).toBeGreaterThanOrEqual(offsets.at(-1) ?? 0)
    const left = (await scrollOf(line)).left
    if (left === offsets.at(-1)) break
    offsets.push(left)
  }
  return offsets
}

/** True when the point at the foot of `line`, where its scrollbar is, hits the line itself. */
const scrollbarTakesPointer = (line: Locator) =>
  line.evaluate((element) => {
    const box = element.getBoundingClientRect()
    const hit = document.elementFromPoint(box.left + box.width / 2, box.bottom - 3)
    return hit === element
  })

for (const { name, query } of TABLES) {
  test.describe(`a pick among a hand of twelve beside a Satchel of twelve, ${name}`, () => {
    for (const prompt of ['bottom', 'discard'] as const) {
      test(`shows every card of a ${prompt} whole at 1644x947, the Satchel scrolling by the wheel`, async ({
        page,
      }) => {
        await page.setViewportSize({ width: 1644, height: 947 })
        await page.goto(`${DEV_WEB_URL}/dev/table?${query}&prompt=${prompt}&hand=12&satchel=12`)
        await expect(handCards(page)).toHaveCount(12)
        await expect(handFan(page)).not.toHaveAttribute('data-scrolls', 'true')
        await expect.poll(() => wholeCards(page)).toBe(12)
        // The Satchel gave way: it scrolls in what the hand leaves, by the wheel and by its scrollbar.
        await expect(satchelFan(page)).toHaveAttribute('data-scrolls', 'true')
        expect(await scrollbarTakesPointer(satchelFan(page))).toBe(true)
        const offsets = await wheelToEnd(page, satchelFan(page))
        expect(offsets.length).toBeGreaterThan(1)
        const end = await scrollOf(satchelFan(page))
        expect(end.left).toBeGreaterThanOrEqual(end.room - 1)
      })
    }

    test('scrolls the hand of a bottom on a phone by the wheel to its last card, which can be chosen', async ({
      page,
    }) => {
      await page.setViewportSize({ width: 390, height: 844 })
      await page.goto(`${DEV_WEB_URL}/dev/table?${query}&prompt=bottom&hand=12&satchel=12`)
      await expect(handCards(page)).toHaveCount(12)
      await expect(handFan(page)).toHaveAttribute('data-scrolls', 'true')
      expect(await scrollbarTakesPointer(handFan(page))).toBe(true)
      const offsets = await wheelToEnd(page, handFan(page))
      expect(offsets.length).toBeGreaterThan(1)
      const end = await scrollOf(handFan(page))
      expect(end.left).toBeGreaterThanOrEqual(end.room - 1)
      const last = handCards(page).last()
      const box = await last.boundingBox()
      expect(box === null ? -1 : box.x + box.width).toBeLessThanOrEqual(390 + 1)
      await last.click()
      await expect(page.getByText(/1 of 2 chosen/).first()).toBeVisible()
    })
  })
}
