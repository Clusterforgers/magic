// Goal: the table is exactly the window at the six sizes of the design (1920x1080, 1440x900, 1366x768, 1280x720, 1024x768 and a 390x844 phone): in a real two-seat game, at the opening hands and at the first priority window, and on the development table at 2, 4, 6 and 12 seats with 40 permanents a seat, the page never scrolls either way, every region of the grid stays inside the window, and your hand, the primary pass button (or Keep), and your life are in view and not covered, no battlefield card in view lies under the prompt sheet or the turn banner, the divider after the commanders stays inside the hand dock; the game-over screen of a real game fits the window too.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { playToTheEnd, Run } from './driver'
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
  /** Battlefield cards in view that the prompt sheet or the turn banner lies over (a card's name and the box that covers it). */
  covered: string[]
  /** How far the divider after the commanders in the hand dock stands above the dock; 0 or less is inside it, null without one. */
  dividerRise: number | null
}

/** What each thing is, as a selector. A prompt's controls are in the dock; the hand is the first card of the row nothing lies over. */
const THINGS = {
  hand: '[data-region="hand"] [data-slot="hand-slot"]:not([data-back]) button[data-object-id]',
  life: '[data-you] [data-slot="life"]',
  primary: '[data-region="controls"] [data-slot="primary-action"]',
  keep: '[data-decision="Mulligan"] button',
  result: '#over-title',
  again: 'main a[href="/new"]',
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
  // A card is in view where no scrolling or clipping ancestor cuts it off; what lies over one is what hides it.
  const inView = (element: Element): Box | null => {
    let { left, top, right, bottom } = element.getBoundingClientRect()
    for (let parent = element.parentElement; parent !== null; parent = parent.parentElement) {
      const style = getComputedStyle(parent)
      if (!/auto|scroll|hidden|clip/.test(style.overflow + style.overflowX + style.overflowY)) continue
      const clip = parent.getBoundingClientRect()
      left = Math.max(left, clip.left)
      top = Math.max(top, clip.top)
      right = Math.min(right, clip.right)
      bottom = Math.min(bottom, clip.bottom)
    }
    return right > left && bottom > top ? { left, top, right, bottom } : null
  }
  const covered: string[] = []
  if (arg.areas.length > 0) {
    const lids = ['[data-slot="shared-sheet"]', '[data-slot="turn-banner"]']
      .map((selector) => document.querySelector(selector))
      .filter((element): element is Element => element !== null)
      // A banner that is not drawn (faded out) lies over nothing.
      .filter((element) => Number(getComputedStyle(element).opacity) > 0)
    for (const card of document.querySelectorAll('[data-slot="battlefield-zone"] button[data-object-id]')) {
      const seen = inView(card)
      if (seen === null) continue
      for (const lid of lids) {
        const over = lid.getBoundingClientRect()
        if (
          Math.min(seen.right, over.right) - Math.max(seen.left, over.left) > 1 &&
          Math.min(seen.bottom, over.bottom) - Math.max(seen.top, over.top) > 1
        ) {
          covered.push(
            `${card.getAttribute('aria-label') ?? 'a card'} under the ${lid.getAttribute('data-slot')}`,
          )
        }
      }
    }
  }
  const divider = document.querySelector('[data-slot="hand-command"]')
  const dock = document.querySelector('[data-area="hand"]')
  return {
    inner: { width: window.innerWidth, height: window.innerHeight },
    scroll: { width: root.scrollWidth, height: root.scrollHeight, y: window.scrollY, x: window.scrollX },
    areas,
    things,
    covered,
    dividerRise:
      divider === null || dock === null
        ? null
        : dock.getBoundingClientRect().top - divider.getBoundingClientRect().top,
  }
}

const AREAS = ['bar', 'opponents', 'shared', 'own', 'hand', 'controls']

const inside = (box: Box, look: Look): boolean =>
  box.left >= -0.5 &&
  box.top >= -0.5 &&
  box.right <= look.inner.width + 0.5 &&
  box.bottom <= look.inner.height + 0.5

interface Where {
  /** What the page is, for the message of a failure. */
  label: string
  /** The page is the table, so its regions are checked and its width class awaited. */
  table: boolean
}

/** Asserts that the page fits the window it is in, and that `needs` are in view and not covered. */
async function expectFits(page: Page, size: Size, needs: readonly Thing[], where: Where): Promise<void> {
  const label = `${where.label} at ${size.width}x${size.height}`
  await page.setViewportSize({ width: size.width, height: size.height })
  if (where.table) {
    await expect(page.locator('[data-slot="table-grid"]')).toHaveAttribute('data-breakpoint', size.breakpoint)
  }
  const things = Object.fromEntries(needs.map((name) => [name, THINGS[name]]))
  const areas = where.table ? AREAS : []
  // The table lays itself out in the frame after the resize; the checks poll until it has.
  await expect
    .poll(
      async () => {
        const look = await page.evaluate(readPage, { things, areas })
        const problems: string[] = []
        if (look.scroll.height > look.inner.height) {
          problems.push(`the page scrolls down: ${look.scroll.height} > ${look.inner.height}`)
        }
        if (look.scroll.width > look.inner.width) {
          problems.push(`the page scrolls sideways: ${look.scroll.width} > ${look.inner.width}`)
        }
        if (look.scroll.y !== 0 || look.scroll.x !== 0) problems.push('the page has been scrolled')
        for (const name of areas) {
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
        problems.push(...look.covered.slice(0, 3))
        if (look.dividerRise !== null && look.dividerRise > 0.5) {
          problems.push(`the divider of the commanders rises ${look.dividerRise}px over the battlefield`)
        }
        return problems
      },
      { message: label, timeout: 5_000 },
    )
    .toEqual([])
}

/** Every size, from the largest to the smallest and back up, so a window that grows is laid out again as well. */
async function atEverySize(page: Page, needs: readonly Thing[], where: Where): Promise<void> {
  for (const size of [...SIZES, ...SIZES.toReversed()]) await expectFits(page, size, needs, where)
}

const onTable = (label: string): Where => ({ label, table: true })

test.describe('the development table with 40 permanents a seat', () => {
  for (const seats of [2, 4, 6]) {
    test(`fits at every size with ${seats} seats and a priority prompt`, async ({ page }) => {
      await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}&prompt=priority&turn=you`)
      await page.locator(THINGS.primary).waitFor()
      await atEverySize(page, ['hand', 'life', 'primary'], onTable(`${seats} seats`))
    })
  }

  test('fits at every size at the opening hands, with the hand in view while the player decides', async ({
    page,
  }) => {
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=4&prompt=mulligan`)
    await page.locator(THINGS.keep).first().waitFor()
    await atEverySize(page, ['hand', 'life', 'keep'], onTable('the opening hands'))
    // Seven cards of the opening hand are in the dock, with the commanders that wait before them in a list of their own.
    await expect(
      page.locator('[data-region="hand"] ul[aria-label$="\'s hand"] button[data-object-id]'),
    ).toHaveCount(7)
  })

  test('fits with a table of twelve seats in focus mode', async ({ page }) => {
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=12&prompt=priority&turn=you`)
    await page.locator(THINGS.primary).waitFor()
    await atEverySize(page, ['hand', 'life', 'primary'], onTable('12 seats'))
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
    await atEverySize(deciding, ['hand', 'life', 'keep'], onTable('the opening hands of a real game'))

    const page = await firstPriority([ada.page, bo.page])
    await atEverySize(page, ['hand', 'life', 'primary'], onTable('the first priority window of a real game'))
    for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})

test('the game-over screen of a real game fits at every size, with the way to a new game in view', async ({
  browser,
}) => {
  test.setTimeout(5 * 60_000)
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  const run = new Run(2_500, 30_000)
  try {
    await everyoneReady(players)
    const [ada, bo] = players
    if (ada === undefined || bo === undefined) throw new Error('two players expected')
    await playToTheEnd(
      [
        { player: ada, style: { blocks: false, autoPay: false, yields: false } },
        { player: bo, style: { blocks: true, autoPay: true, yields: true } },
      ],
      run,
    )
    await atEverySize(ada.page, ['result', 'again'], { label: 'the game-over screen', table: false })
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
