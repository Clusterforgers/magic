// Goal: the table is exactly the window at the six sizes of the design (1920x1080, 1440x900, 1366x768, 1280x720, 1024x768 and a 390x844 phone): in a real two-seat game, at the opening hands and at the first priority window, and on the development table at 2, 4 and 6 seats with 40 permanents a seat, the page never scrolls either way, every region of the grid stays inside the window, and your hand, the primary pass button (or Keep), and your life are in view and not covered.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { createAndJoin, everyoneReady } from './lobby'
import { DEV_WEB_URL } from './ports'

type Breakpoint = 'xs' | 's' | 'm' | 'l' | 'xl'

interface Size {
  width: number
  height: number
  /** The width class the table should be in (`regionSizes` in `layout.ts`). */
  breakpoint: Breakpoint
}

const SIZES: readonly Size[] = [
  { width: 1920, height: 1080, breakpoint: 'xl' },
  { width: 1440, height: 900, breakpoint: 'l' },
  { width: 1366, height: 768, breakpoint: 'l' },
  { width: 1280, height: 720, breakpoint: 'l' },
  { width: 1024, height: 768, breakpoint: 'm' },
  { width: 390, height: 844, breakpoint: 'xs' },
]

interface Box {
  left: number
  top: number
  right: number
  bottom: number
}

/** What a page says about where things are, read in one step so it cannot change halfway. */
interface Look {
  inner: { width: number; height: number }
  scroll: { width: number; height: number; y: number; x: number }
  /** Each region of the grid by name. */
  areas: Record<string, Box | null>
  /** The things that must be in view, by name. */
  things: Record<string, { box: Box; free: boolean } | null>
}

/** What each thing is, as a selector. A prompt's controls are in the dock; the hand is the dock's first card. */
const THINGS = {
  hand: '[data-region="hand"] button[data-object-id]',
  life: '[data-you] [data-slot="life"]',
  primary: '[data-region="controls"] [data-slot="primary-action"]',
  keep: '[data-decision="Mulligan"] button',
} as const

type Thing = keyof typeof THINGS

/**
 * Runs in the page. `free` is whether the topmost element at the centre of the thing is the thing or inside it, so
 * a prompt sheet or another region lying over it shows up.
 */
function readPage(arg: { things: Record<string, string>; areas: string[] }): Look {
  const box = (element: Element): Box => {
    const { left, top, right, bottom } = element.getBoundingClientRect()
    return { left, top, right, bottom }
  }
  const root = document.documentElement
  const areas: Look['areas'] = {}
  for (const name of arg.areas) {
    const element = document.querySelector(`[data-area="${name}"]`)
    areas[name] = element === null ? null : box(element)
  }
  const things: Look['things'] = {}
  for (const [name, selector] of Object.entries(arg.things)) {
    const element = document.querySelector(selector)
    if (element === null) {
      things[name] = null
      continue
    }
    const rect = box(element)
    const top = document.elementFromPoint((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2)
    things[name] = { box: rect, free: top !== null && element.contains(top) }
  }
  return {
    inner: { width: window.innerWidth, height: window.innerHeight },
    scroll: { width: root.scrollWidth, height: root.scrollHeight, y: window.scrollY, x: window.scrollX },
    areas,
    things,
  }
}

const AREAS = ['bar', 'opponents', 'shared', 'own', 'hand', 'controls']

/** Gives the page this window size and waits until the table has laid itself out for it. */
async function resizeTo(page: Page, size: Size): Promise<void> {
  await page.setViewportSize({ width: size.width, height: size.height })
  await expect(page.locator('[data-slot="table-grid"]')).toHaveAttribute('data-breakpoint', size.breakpoint)
}

const inside = (box: Box, look: Look): boolean =>
  box.left >= -0.5 &&
  box.top >= -0.5 &&
  box.right <= look.inner.width + 0.5 &&
  box.bottom <= look.inner.height + 0.5

/** Asserts that the table fits the window it is in, and that `needs` are in view and not covered. */
async function expectFits(page: Page, size: Size, needs: readonly Thing[], where: string): Promise<void> {
  const label = `${where} at ${size.width}x${size.height}`
  await resizeTo(page, size)
  const things = Object.fromEntries(needs.map((name) => [name, THINGS[name]]))
  // The table lays itself out in the frame after the resize; the checks poll until it has.
  await expect
    .poll(
      async () => {
        const look = await page.evaluate(readPage, { things, areas: AREAS })
        const problems: string[] = []
        if (look.scroll.height > look.inner.height) {
          problems.push(`the page scrolls down: ${look.scroll.height} > ${look.inner.height}`)
        }
        if (look.scroll.width > look.inner.width) {
          problems.push(`the page scrolls sideways: ${look.scroll.width} > ${look.inner.width}`)
        }
        if (look.scroll.y !== 0 || look.scroll.x !== 0) problems.push('the page has been scrolled')
        for (const name of AREAS) {
          const area = look.areas[name]
          if (area === null || area === undefined) problems.push(`the ${name} region is missing`)
          else if (!inside(area, look)) problems.push(`the ${name} region leaves the window`)
        }
        for (const name of needs) {
          const thing = look.things[name]
          if (thing === null || thing === undefined) problems.push(`${name} is missing`)
          else if (!inside(thing.box, look)) problems.push(`${name} is outside the window`)
          else if (!thing.free) problems.push(`${name} is covered`)
        }
        return problems
      },
      { message: label, timeout: 5_000 },
    )
    .toEqual([])
}

/** Every size, from the largest to the smallest and back up, so a window that grows is laid out again as well. */
async function atEverySize(page: Page, needs: readonly Thing[], where: string): Promise<void> {
  for (const size of [...SIZES, ...SIZES.toReversed()]) await expectFits(page, size, needs, where)
}

test.describe('the development table with 40 permanents a seat', () => {
  for (const seats of [2, 4, 6]) {
    test(`fits at every size with ${seats} seats and a priority prompt`, async ({ page }) => {
      await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}&prompt=priority&turn=you`)
      await page.locator(THINGS.primary).waitFor()
      await atEverySize(page, ['hand', 'life', 'primary'], `${seats} seats`)
    })
  }

  test('fits at every size at the opening hands, with the hand in view while the player decides', async ({
    page,
  }) => {
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=4&prompt=mulligan`)
    await page.locator(THINGS.keep).first().waitFor()
    await atEverySize(page, ['hand', 'life', 'keep'], 'the opening hands')
    // Seven cards of the opening hand are in the dock.
    await expect(page.locator('[data-region="hand"] ul button[data-object-id]')).toHaveCount(7)
  })

  test('fits with a table of twelve seats in focus mode', async ({ page }) => {
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=12&prompt=priority&turn=you`)
    await page.locator(THINGS.primary).waitFor()
    await atEverySize(page, ['hand', 'life', 'primary'], '12 seats')
  })
})

/** The page of the seat that is asked this decision now, once one is; the seat that goes first decides first. */
async function askedFor(pages: readonly Page[], decision: 'Mulligan' | 'Priority'): Promise<Page> {
  const prompts = pages.map((page) => page.locator(`[data-decision="${decision}"]`))
  let asked = -1
  await expect
    .poll(
      async () => {
        for (const [index, prompt] of prompts.entries()) {
          if ((await prompt.count()) > 0) asked = index
        }
        return asked
      },
      { timeout: 45_000 },
    )
    .toBeGreaterThanOrEqual(0)
  const page = pages[asked]
  if (page === undefined) throw new Error('nobody was asked')
  return page
}

/** Both seats of a new game keep their hands; resolves with the page of the seat that is asked for priority first. */
async function firstPriority(pages: readonly Page[]): Promise<Page> {
  await Promise.all(pages.map((page) => page.getByRole('button', { name: 'Keep', exact: true }).click()))
  return askedFor(pages, 'Priority')
}

test('a real two-seat game fits at every size, at the opening hands and at the first window', async ({
  browser,
}) => {
  test.setTimeout(3 * 60_000)
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  try {
    await everyoneReady(players)
    const [ada, bo] = players
    if (ada === undefined || bo === undefined) throw new Error('two players expected')
    // The opening hand is seven cards in the hand dock and the Keep button is in view, whatever the window.
    const deciding = await askedFor([ada.page, bo.page], 'Mulligan')
    await expect(deciding.locator('[data-region="hand"] ul button[data-object-id]')).toHaveCount(7)
    await atEverySize(deciding, ['hand', 'life', 'keep'], 'the opening hands of a real game')

    const page = await firstPriority([ada.page, bo.page])
    await atEverySize(page, ['hand', 'life', 'primary'], 'the first priority window of a real game')
    for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
