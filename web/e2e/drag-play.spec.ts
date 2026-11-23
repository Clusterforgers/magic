// Goal: on the development table at 1280x720 and 1920x1080 a land, then a spell, dragged with the mouse up out of the hand onto the board is played on the drop (the board is marked as the drop target and the card says "Release to play"), a card that cannot be played says so and returns with the decision still open, a card brought back over the hand plays nothing, a drag along the hand still reorders and plays nothing, a card with two plays opens the chooser a click opens, a commander is cast from the head of the dock at four seats, and on a touch screen a held card is played the same way.
import { expect, test } from '@playwright/test'
import type { Locator, Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'

const SCENE = `${DEV_WEB_URL}/dev/table?seats=2&turn=you&prompt=priority&hand=7`
const HAND = '[data-region="hand"] ul[aria-label$="\'s hand"]'
const CARDS = `${HAND} button[data-object-id]`
const FACE = '[data-slot="hand-drag-face"]'
const TARGET = '[data-slot="hand-play-target"]'

const SIZES = [
  { width: 1280, height: 720 },
  { width: 1920, height: 1080 },
]

/** The object ids of the hand's cards, left to right. */
const order = (page: Page): Promise<string[]> =>
  page.locator(CARDS).evaluateAll((cards) => cards.map((card) => card.getAttribute('data-object-id') ?? ''))

/** Opens `url` with the hand laid out. */
async function open(page: Page, url: string): Promise<void> {
  await page.goto(url)
  await expect(page.locator(CARDS).first()).toBeVisible()
  await page.waitForFunction(
    (hand) => document.querySelector<HTMLElement>(hand)?.style.getPropertyValue('--card-w') !== '',
    HAND,
  )
  await expect(page.locator('[data-decision]')).not.toHaveCount(0)
}

/** The middle of the part of a card its line shows. */
async function centre(card: Locator): Promise<{ x: number; y: number }> {
  return card.evaluate((element) => {
    const box = element.getBoundingClientRect()
    const line = element.closest('ul')?.getBoundingClientRect() ?? box
    const left = Math.max(box.left, line.left)
    const right = Math.min(box.right, line.right)
    return { x: (left + right) / 2, y: box.top + box.height / 2 }
  })
}

/** A point on the viewer's board, well above the hand dock. */
async function onBoard(page: Page): Promise<{ x: number; y: number }> {
  return page.locator('[data-region="hand"]').evaluate((dock) => {
    const box = dock.getBoundingClientRect()
    return { x: box.left + box.width / 2, y: box.top - 120 }
  })
}

/** Picks the card up with the mouse and carries it onto the board, in steps as a hand moves; the button stays down. */
async function carryUp(page: Page, card: Locator): Promise<void> {
  const from = await centre(card)
  const to = await onBoard(page)
  await page.mouse.move(from.x, from.y)
  await page.mouse.down()
  await page.mouse.move(from.x, from.y - 10, { steps: 3 })
  await page.mouse.move(to.x, to.y, { steps: 12 })
}

/** The first card of the hand that the prompt marks legal and whose spoken name matches `name`. */
const legal = (page: Page, name: RegExp): Locator =>
  page.locator(`${CARDS}[data-highlight="legal"]`).and(page.getByRole('button', { name })).first()

for (const { width, height } of SIZES) {
  test.describe(`drag to play at ${width}x${height}`, () => {
    test.use({ viewport: { width, height } })

    for (const [kind, name] of [
      ['a land', /\bLand\b/],
      ['a spell', /\b(Creature|Instant|Sorcery|Enchantment|Artifact)\b/],
    ] as const) {
      test(`${kind} dragged onto the board is played on the drop`, async ({ page }) => {
        await open(page, SCENE)
        const card = legal(page, name)
        const id = await card.getAttribute('data-object-id')
        await carryUp(page, card)
        await expect(page.locator(FACE)).toHaveAttribute('data-play', 'can')
        await expect(page.locator(FACE)).toContainText('Release to play')
        await expect(page.locator(`${TARGET}[data-over]`)).toBeVisible()
        // Nothing is answered while the card is in the air.
        await expect(page.locator('[data-decision]')).not.toHaveCount(0)
        await page.screenshot({ path: `test-results/drag-play-${kind.replace(' ', '-')}-${width}.png` })
        await page.mouse.up()
        await expect(page.locator('[data-decision]')).toHaveCount(0)
        await expect(page.locator(FACE)).toHaveCount(0)
        await expect(page.locator(TARGET)).toHaveCount(0)
        expect(id).not.toBeNull()
      })
    }

    test('a card that cannot be played says so and returns; one brought back over the hand plays nothing', async ({
      page,
    }) => {
      await open(page, SCENE)
      const start = await order(page)
      const dimmed = page.locator(`${CARDS}[data-highlight="dimmed"]`).first()
      await carryUp(page, dimmed)
      await expect(page.locator(FACE)).toHaveAttribute('data-play', 'cannot')
      await expect(page.locator(TARGET)).toHaveCount(0)
      await page.screenshot({ path: `test-results/drag-play-cannot-${width}.png` })
      await page.mouse.up()
      await expect(page.locator(FACE)).toHaveCount(0)
      await expect(page.locator('[data-decision]')).not.toHaveCount(0)

      // A playable card carried up and back down over its own place: let go there, it stays in the hand.
      const card = legal(page, /\bLand\b/)
      const home = await centre(card)
      await carryUp(page, card)
      await expect(page.locator(FACE)).toHaveAttribute('data-play', 'can')
      await page.mouse.move(home.x, home.y, { steps: 12 })
      await expect(page.locator(FACE)).not.toHaveAttribute('data-play')
      await expect(page.locator(`${TARGET}[data-over]`)).toHaveCount(0)
      await page.mouse.up()
      await expect(page.locator(FACE)).toHaveCount(0)
      await expect(page.locator('[data-decision]')).not.toHaveCount(0)
      expect(await order(page)).toEqual(start)
    })

    test('a drag along the hand reorders and plays nothing', async ({ page }) => {
      await open(page, SCENE)
      const start = await order(page)
      const a = await centre(page.locator(`${CARDS}[data-object-id="${start[6]}"]`))
      const b = await centre(page.locator(`${CARDS}[data-object-id="${start[0]}"]`))
      await page.mouse.move(a.x, a.y)
      await page.mouse.down()
      await page.mouse.move(a.x - 10, a.y, { steps: 3 })
      await page.mouse.move(b.x, b.y, { steps: 12 })
      await expect(page.locator('[data-slot="hand-drop-mark"]')).toHaveCount(1)
      await expect(page.locator(FACE)).not.toHaveAttribute('data-play')
      await page.mouse.up()
      const want = [start[6], ...start.slice(0, 6)]
      await expect.poll(() => order(page)).toEqual(want)
      await expect(page.locator('[data-decision]')).not.toHaveCount(0)
    })

    test('a card with two plays dropped on the board opens the chooser a click opens', async ({ page }) => {
      await open(page, `${DEV_WEB_URL}/dev/table?seats=2&prompt=priority-cycling`)
      const angel = page.locator(`${CARDS}[aria-label^="Angel of the Ruins"]`).first()
      await carryUp(page, angel)
      await expect(page.locator(FACE)).toHaveAttribute('data-play', 'can')
      await page.mouse.up()
      const chooser = page.locator('[data-slot="ability-chooser"]')
      await expect(chooser).toBeVisible()
      await expect(chooser.locator('[data-play="cast"]')).toContainText('Cast Angel of the Ruins')
      await expect(chooser.getByRole('menuitem', { name: 'Activate Plainscycling 2' })).toBeVisible()
      await expect(page.locator('[data-decision]')).not.toHaveCount(0)
    })

    test('a commander at the head of the dock is cast by a drag at four seats', async ({ page }) => {
      await open(page, `${DEV_WEB_URL}/dev/table?seats=4&turn=you&prompt=priority&hand=7`)
      const commander = page
        .locator('[data-slot="hand-command"] [data-slot="hand-slot"][data-commander] button[data-object-id]')
        .first()
      await expect(commander).toHaveAttribute('data-highlight', 'legal')
      await carryUp(page, commander)
      await expect(page.locator(FACE)).toHaveAttribute('data-play', 'can')
      await expect(page.locator(`${TARGET}[data-over]`)).toBeVisible()
      await page.mouse.up()
      await expect(page.locator('[data-decision]')).toHaveCount(0)
    })
  })
}

test('a touch plays a held hand card dragged onto the board', async ({ browser }) => {
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true })
  const page = await context.newPage()
  await open(page, SCENE)
  await page.locator(HAND).evaluate((list) => list.scrollTo({ left: 0 }))
  const card = legal(page, /\bLand\b/)
  await card.scrollIntoViewIfNeeded()
  const from = await centre(card)
  const to = await onBoard(page)
  const session = await context.newCDPSession(page)
  const touch = (type: 'touchStart' | 'touchMove' | 'touchEnd', x: number, y: number) =>
    session.send('Input.dispatchTouchEvent', {
      type,
      touchPoints: type === 'touchEnd' ? [] : [{ x: Math.round(x), y: Math.round(y) }],
    })
  await touch('touchStart', from.x, from.y)
  await page.waitForTimeout(400)
  for (let step = 1; step <= 8; step += 1)
    await touch('touchMove', from.x + ((to.x - from.x) * step) / 8, from.y + ((to.y - from.y) * step) / 8)
  await expect(page.locator(FACE)).toHaveAttribute('data-play', 'can')
  await touch('touchEnd', to.x, to.y)
  await expect(page.locator('[data-decision]')).toHaveCount(0)
  await context.close()
})
