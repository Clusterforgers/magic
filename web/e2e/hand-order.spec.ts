// Goal: on the development table at 1400x900 and 390x844 the viewer drags a card of the hand in front of another with the mouse (and on a touch screen after a short hold) without playing it, a swipe still moves no card, the table menu's "Hand order" puts the hand in "Lands, creatures, spells" order and back to "As drawn", and a custom order lasts through a reload.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SCENE = `${DEV_WEB_URL}/dev/table?seats=2&turn=you&prompt=priority&hand=7`
const HAND = '[data-region="hand"] ul[aria-label$="\'s hand"]'

/** A size, and which card is dragged in front of which: on a phone two cards the line shows at once. */
const CASES = [
  { width: 1400, height: 900, from: 6, to: 0 },
  { width: 390, height: 844, from: 1, to: 0 },
]

/** The object ids of the hand's cards, left to right. */
const order = (page: Page): Promise<string[]> =>
  page
    .locator(`${HAND} button[data-object-id]`)
    .evaluateAll((cards) => cards.map((card) => card.getAttribute('data-object-id') ?? ''))

/** 0 for a land, 1 for a creature, 2 for any other card, read from each card's spoken name. */
const groups = (page: Page): Promise<number[]> =>
  page.locator(`${HAND} button[data-object-id]`).evaluateAll((cards) =>
    cards.map((card) => {
      const label = card.getAttribute('aria-label') ?? ''
      return /\bLand\b/.test(label) ? 0 : /\bCreature\b/.test(label) ? 1 : 2
    }),
  )

/** Opens the table with the hand laid out and its line scrolled to the start; the hand's ids in order. */
async function open(page: Page): Promise<string[]> {
  await page.goto(SCENE)
  await expect(page.locator(`${HAND} button[data-object-id]`)).toHaveCount(7)
  await page.waitForFunction(
    (hand) => document.querySelector<HTMLElement>(hand)?.style.getPropertyValue('--card-w') !== '',
    HAND,
  )
  await page.locator(HAND).evaluate((list) => list.scrollTo({ left: 0 }))
  return order(page)
}

/** The middle of the part of the card with id `id` that the hand's line shows; nothing scrolls. */
async function centre(page: Page, id: string): Promise<{ x: number; y: number }> {
  return page.locator(`${HAND} button[data-object-id="${id}"]`).evaluate((card) => {
    const box = card.getBoundingClientRect()
    const line = card.closest('ul')?.getBoundingClientRect() ?? box
    const left = Math.max(box.left, line.left)
    const right = Math.min(box.right, line.right)
    return { x: (left + right) / 2, y: box.top + box.height / 2 }
  })
}

/**
 * Scrolls the hand's line so the edge between the card with id `id` and the one before it is in the middle: a phone's
 * line can be narrower than a card, and so shows a part of both.
 */
async function showEdge(page: Page, id: string): Promise<void> {
  await page.locator(`${HAND} button[data-object-id="${id}"]`).evaluate((card) => {
    const list = card.closest('ul')
    if (list === null) return
    const left = card.getBoundingClientRect().left - list.getBoundingClientRect().left + list.scrollLeft
    list.scrollTo({ left: left - list.clientWidth / 2 })
  })
}

/** `start` with the card at `from` put at `to`. */
function moved(start: readonly string[], from: number, to: number): string[] {
  const next = [...start]
  const [card] = next.splice(from, 1)
  next.splice(to, 0, card ?? '')
  return next
}

/** The order in force by its label: the dock's sort button names it; a phone's table menu presses it (Custom: none). */
async function expectMode(page: Page, label: string): Promise<void> {
  const button = page.getByRole('button', { name: /^Sort hand:/ })
  if ((await button.count()) > 0) {
    await expect(button).toHaveAccessibleName(`Sort hand: ${label}`)
    return
  }
  await page.getByRole('button', { name: 'Table menu' }).click()
  const pressed = page.getByRole('group', { name: 'Hand order' }).locator('button[aria-pressed="true"]')
  if (label === 'Custom') await expect(pressed).toHaveCount(0)
  else await expect(pressed).toHaveText(label)
  await page.keyboard.press('Escape')
}

/** Picks an auto sort: from the dock's sort button, or on a phone from the table menu's "Hand order". */
async function chooseSort(page: Page, label: string): Promise<void> {
  const button = page.getByRole('button', { name: /^Sort hand:/ })
  if ((await button.count()) > 0) {
    await button.click()
    await page.getByRole('menuitemradio', { name: label }).click()
  } else {
    await page.getByRole('button', { name: 'Table menu' }).click()
    await page.getByRole('group', { name: 'Hand order' }).getByRole('button', { name: label }).click()
    await page.keyboard.press('Escape')
  }
  await expectMode(page, label)
}

for (const { width, height, from, to } of CASES) {
  test(`a hand card is dragged before another and the hand sorts at ${width}x${height}`, async ({ page }) => {
    await page.setViewportSize({ width, height })
    const start = await open(page)
    await showEdge(page, start[Math.max(from, to)] ?? '')
    const a = await centre(page, start[from] ?? '')
    const b = await centre(page, start[to] ?? '')
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    // Past the drag threshold, then onto the other card in steps, as a hand moves.
    await page.mouse.move(a.x - 10, a.y, { steps: 3 })
    await page.mouse.move(b.x, b.y, { steps: 12 })
    await expect(page.locator('[data-slot="hand-drop-mark"]')).toHaveCount(1)
    await expect(page.locator('[data-slot="hand-drag-face"]')).toHaveCount(1)
    await page.mouse.up()
    // The card moved in front and stayed in the hand: the drop played nothing.
    const want = moved(start, from, to)
    await expect.poll(() => order(page)).toEqual(want)
    await expect(page.locator('[data-slot="hand-drag-face"]')).toHaveCount(0)
    await expectMode(page, 'Custom')

    await page.reload()
    await expect(page.locator(`${HAND} button[data-object-id]`)).toHaveCount(7)
    await expect.poll(() => order(page)).toEqual(want)

    await chooseSort(page, 'Lands, creatures, spells')
    const sorted = await groups(page)
    expect(sorted).toEqual([...sorted].sort((x, y) => x - y))
    expect(sorted[0]).toBe(0)

    await chooseSort(page, 'As drawn')
    await expect.poll(() => order(page)).toEqual(start)
  })
}

test('a touch drags a hand card after a short hold, and a swipe does not', async ({ browser }) => {
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true })
  const page = await context.newPage()
  const start = await open(page)
  const cdp = await context.newCDPSession(page)
  const touch = (type: 'touchStart' | 'touchMove' | 'touchEnd', point?: { x: number; y: number }) =>
    cdp.send('Input.dispatchTouchEvent', {
      type,
      touchPoints: point === undefined ? [] : [{ x: point.x, y: point.y }],
    })
  // A quick swipe moves no card.
  const swipe = await centre(page, start[0] ?? '')
  await touch('touchStart', swipe)
  for (let step = 1; step <= 5; step += 1) await touch('touchMove', { x: swipe.x + step * 6, y: swipe.y })
  await touch('touchEnd')
  await expect(page.locator('[data-slot="hand-drag-face"]')).toHaveCount(0)
  expect(await order(page)).toEqual(start)
  await showEdge(page, start[1] ?? '')

  // Held still past the hold, then moved: the second card goes in front of the first.
  const a = await centre(page, start[1] ?? '')
  const b = await centre(page, start[0] ?? '')
  await touch('touchStart', a)
  await page.waitForTimeout(400)
  for (let step = 1; step <= 10; step += 1) {
    await touch('touchMove', { x: a.x + ((b.x - a.x) * step) / 10, y: a.y })
  }
  await expect(page.locator('[data-slot="hand-drag-face"]')).toHaveCount(1)
  await touch('touchEnd')
  await expect.poll(() => order(page)).toEqual(moved(start, 1, 0))
  await context.close()
})
