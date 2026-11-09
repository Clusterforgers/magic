// Goal: on the development table at 1400x900 and 390x844 the viewer drags a card of the hand in front of another with the mouse (and on a touch screen after a short hold) without playing it, the sort menu puts the hand in "Lands, creatures, spells" order and back to "As drawn", and a custom order lasts through a reload.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SCENE = `${DEV_WEB_URL}/dev/table?seats=2&turn=you&prompt=priority&hand=7`
const HAND = '[data-region="hand"] ul[aria-label$="\'s hand"]'
const SIZES = [
  { width: 1400, height: 900 },
  { width: 390, height: 844 },
]

/** The object ids of the hand's cards, left to right. */
const order = (page: Page): Promise<string[]> =>
  page
    .locator(`${HAND} button[data-object-id]`)
    .evaluateAll((cards) => cards.map((card) => card.getAttribute('data-object-id') ?? ''))

/** 0 for a land, 1 for a creature, 2 for any other card, read from each card's spoken name. */
const groups = (page: Page): Promise<number[]> =>
  page
    .locator(`${HAND} button[data-object-id]`)
    .evaluateAll((cards) =>
      cards.map((card) => {
        const label = card.getAttribute('aria-label') ?? ''
        return /\bLand\b/.test(label) ? 0 : /\bCreature\b/.test(label) ? 1 : 2
      }),
    )

async function open(page: Page): Promise<string[]> {
  await page.goto(SCENE)
  await expect(page.locator(`${HAND} button[data-object-id]`)).toHaveCount(7)
  await page.waitForFunction(
    (hand) => document.querySelector<HTMLElement>(hand)?.style.getPropertyValue('--card-w') !== '',
    HAND,
  )
  return order(page)
}

/** The centre of the card with id `id`, scrolled into view. */
async function centre(page: Page, id: string): Promise<{ x: number; y: number }> {
  const card = page.locator(`${HAND} button[data-object-id="${id}"]`)
  await card.scrollIntoViewIfNeeded()
  const box = await card.boundingBox()
  if (box === null) throw new Error(`card ${id} has no box`)
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 }
}

async function chooseSort(page: Page, label: string): Promise<void> {
  await page.getByRole('button', { name: /^Sort hand:/ }).click()
  await page.getByRole('menuitemradio', { name: label }).click()
  await expect(page.getByRole('button', { name: `Sort hand: ${label}` })).toBeVisible()
}

for (const size of SIZES) {
  test(`a hand card is dragged before another and the hand sorts at ${size.width}x${size.height}`, async ({
    page,
  }) => {
    await page.setViewportSize(size)
    const start = await open(page)
    const last = start[6] ?? ''
    const first = start[0] ?? ''
    const from = await centre(page, last)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    // Past the drag threshold, then onto the first card in steps, as a hand moves.
    await page.mouse.move(from.x - 10, from.y, { steps: 3 })
    const to = await centre(page, first)
    await page.mouse.move(to.x, to.y, { steps: 12 })
    await expect(page.locator('[data-slot="hand-drop-mark"]')).toHaveCount(1)
    await page.mouse.up()
    // The card moved in front and stayed in the hand: the drop played nothing.
    await expect.poll(() => order(page)).toEqual([last, ...start.slice(0, 6)])
    await expect(page.getByRole('button', { name: 'Sort hand: Custom' })).toBeVisible()
    await expect(page.locator('[data-slot="hand-drag-face"]')).toHaveCount(0)

    await page.reload()
    await expect(page.locator(`${HAND} button[data-object-id]`)).toHaveCount(7)
    await expect.poll(() => order(page)).toEqual([last, ...start.slice(0, 6)])

    await chooseSort(page, 'Lands, creatures, spells')
    const sorted = await groups(page)
    expect(sorted).toEqual([...sorted].sort((a, b) => a - b))
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
  const second = start[1] ?? ''
  const fourth = start[3] ?? ''
  const from = await centre(page, fourth)
  const to = await centre(page, second)
  // Held still past the hold, then moved: the card goes in front of the second.
  await touch('touchStart', from)
  await page.waitForTimeout(400)
  for (let step = 1; step <= 10; step += 1) {
    await touch('touchMove', { x: from.x + ((to.x - from.x) * step) / 10, y: from.y })
  }
  await touch('touchEnd')
  await expect
    .poll(() => order(page))
    .toEqual([start[0], fourth, second, ...start.slice(2, 3), ...start.slice(4)])
  // A quick swipe moves no card.
  const now = await order(page)
  const swipe = await centre(page, now[5] ?? '')
  await touch('touchStart', swipe)
  for (let step = 1; step <= 5; step += 1) await touch('touchMove', { x: swipe.x - step * 20, y: swipe.y })
  await touch('touchEnd')
  await page.waitForTimeout(300)
  expect(await order(page)).toEqual(now)
  await context.close()
})
