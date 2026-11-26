// Goal: the table is exactly the window at the six sizes of the design (1920x1080, 1440x900, 1366x768, 1280x720, 1024x768 and a 390x844 phone): in a real two-seat game, at the opening hands and at the first priority window, and on the development table at 2, 4, 6 and 12 seats, and at 16, 24 and 40 in both themes, with 40 permanents a seat, and also at 2560x1440, 1600x1000, 1400x900, 768x1024 and 360x740, the page never scrolls either way, every region of the grid stays inside the window, and your hand, the primary pass button (or Keep), and your life are in view and not covered, no battlefield card in view lies under the prompt sheet or the turn banner, the divider after the commanders stays inside the hand dock, three targeting items on the stack at rest fit too; the game-over screen of a real game fits the window too; with the attachments scene (`equip=1`, also on the duel board) the table fits as well, every grant mark is written in whole words of 11 px or more, and a pod column draws its corner as pictures inside it or as chips with no card; on the crowded scene (`crowd=1`) the stack's chips and its "+N more" button stay inside the strip with every item counted once, and the three bundles of the big attack carry whole labels in the window and off the strip. In every game mode with bare boards (duel, three to twelve seats, teams, Two-Headed Giant, boss fights of one to four heroes and with minions) the opening hand stands centred over the table between the top bar and the dock, every card whole in one row, its answers clear of the sentence and the panel not scrolling, as in a real game; and on an empty first turn the opponents' row keeps a quarter of the boards' room and never outgrows yours. At 1280x720 to 1920x1080 your board keeps its cards clear of the grown controls dock in the duel, at four seats and in a boss fight, a Satchel of twelve leaves the hand two whole cards, a prompt card under 76 px draws the compact face whole, and a card prompt shows its question and a first row of whole cards in the Stack panel and the window, wrapping more than eight; an eight-seat grid shows a row of each opponent's cards, and your pod column in boss mode shows every chip without scrolling.
import { expect, test } from './test'
import type { Page } from '@playwright/test'
import { playToTheEnd, Run } from './driver'
import { createAndJoin, everyoneReady, reseat } from './lobby'
import { expectTheme, themedContext } from './layoutGate'
import { boardRows, EMPTY_ROW_LEAST_SHARE, openingProblems, TABLE_MODES } from './openingFit'
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
  /** The combat lines that run over your life figure (each line's key). */
  overLife: string[]
  /** How many px the controls dock and the hand dock scroll inside themselves; null off the table. */
  dockScroll: { controls: number; hand: number } | null
}

/**
 * What each thing is, as a selector. A prompt's controls are in the dock; the hand is the first card of the row nothing
 * lies over, or the first of the opening hand where the centre overlay carries it and the dock hides its copy.
 */
const THINGS = {
  hand: '[data-region="hand"] [data-slot="hand-slot"]:not([data-back]):not([data-floating] *) button[data-object-id], [data-opening] [data-slot="opening-card"]',
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
  // The lines are drawn over a pointer-less layer, so `elementFromPoint` never sees them: follow each path instead.
  const overLife: string[] = []
  const life = document.querySelector('[data-you] [data-slot="life"]')?.getBoundingClientRect()
  if (life !== undefined) {
    for (const group of document.querySelectorAll('[data-layer="lines"] [data-line]')) {
      const path = group.querySelector<SVGPathElement>('.combat-line')
      const matrix = path?.getScreenCTM()
      if (path === null || path === undefined || matrix === null) continue
      const length = path.getTotalLength()
      for (let step = 0; step <= 40; step += 1) {
        const at = path.getPointAtLength((length * step) / 40).matrixTransform(matrix)
        if (at.x >= life.left && at.x <= life.right && at.y >= life.top && at.y <= life.bottom) {
          overLife.push(group.getAttribute('data-line') ?? 'a line')
          break
        }
      }
    }
  }
  const divider = document.querySelector('[data-slot="hand-command"]')
  const dock = document.querySelector('[data-area="hand"]')
  const inner = (selector: string): number => {
    const element = document.querySelector(selector)
    return element === null ? 0 : element.scrollHeight - element.clientHeight
  }
  return {
    inner: { width: window.innerWidth, height: window.innerHeight },
    scroll: { width: root.scrollWidth, height: root.scrollHeight, y: window.scrollY, x: window.scrollX },
    areas,
    things,
    covered,
    overLife,
    dockScroll:
      arg.areas.length === 0
        ? null
        : { controls: inner('[data-region="controls"]'), hand: inner('[data-region="hand"]') },
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
        for (const line of look.overLife.slice(0, 3))
          problems.push(`the combat line ${line} runs over your life`)
        if (look.dividerRise !== null && look.dividerRise > 0.5) {
          problems.push(`the divider of the commanders rises ${look.dividerRise}px over the battlefield`)
        }
        for (const [name, scrolled] of Object.entries(look.dockScroll ?? {})) {
          if (scrolled > 1) problems.push(`the ${name} dock scrolls inside itself by ${scrolled}px`)
        }
        return problems
      },
      { message: label, timeout: 5_000 },
    )
    .toEqual([])
}

/** The widths between and beyond the six: a wide window, a laptop, a tablet and a small phone. */
const EXTRA_SIZES: readonly Size[] = [
  { width: 2560, height: 1440, breakpoint: 'xl' },
  { width: 1600, height: 1000, breakpoint: 'xl' },
  { width: 1400, height: 900, breakpoint: 'l' },
  { width: 768, height: 1024, breakpoint: 's' },
  { width: 360, height: 740, breakpoint: 'xs' },
]

const SWEEP: readonly Size[] = [...SIZES, ...EXTRA_SIZES]

/** Every size, from the largest to the smallest and back up, so a window that grows is laid out again as well. */
async function atEverySize(
  page: Page,
  needs: readonly Thing[],
  where: Where,
  also?: (size: Size) => Promise<void>,
): Promise<void> {
  for (const size of [...SWEEP, ...SWEEP.toReversed()]) {
    await expectFits(page, size, needs, where)
    await also?.(size)
  }
}

/** Polls until the floating opening hand keeps its rules at `size` (`openingProblems`). */
async function expectOpening(page: Page, size: Size, label: string): Promise<void> {
  await expect
    .poll(() => openingProblems(page), {
      message: `${label} at ${size.width}x${size.height}`,
      timeout: 5_000,
    })
    .toEqual([])
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
    // The opening hands come before any permanent, so the boards are bare as in a real game (`bare=1`): a phone's sheet
    // grows over the opponents' board for the hand (docs/web.md, Prompt band), which holds nothing yet.
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=4&prompt=mulligan&bare=1`)
    await page.locator(THINGS.keep).first().waitFor()
    await atEverySize(page, ['hand', 'life', 'keep'], onTable('the opening hands'))
    // Seven cards of the opening hand are in the dock, with the commanders that wait before them in a list of their own.
    await expect(
      page.locator('[data-region="hand"] ul[aria-label$="\'s hand"] button[data-object-id]'),
    ).toHaveCount(7)
  })

  test('draws the duel board of an early Commander game large, in lanes that fit without scrolling', async ({
    page,
  }) => {
    await page.goto(`${DEV_WEB_URL}/dev/table?board=duel&prompt=priority&turn=you`)
    await page.locator(THINGS.primary).waitFor()
    await atEverySize(page, ['hand', 'life', 'primary'], onTable('the duel board'))
    for (const [width, height, own, theirs] of [
      // The band keeps the ticker's line above it (16 px): the opponent's row gives a rung (148 of the ladder) at 1920,
      // yours at 1440, where the opponent's pod column also keeps its pictures (`POD_PICTURES_PX`).
      [1920, 1080, 190, 148],
      [1440, 900, 148, 116],
      // A three-line prompt stands above the strip here: the opponent keeps the name and P/T face.
      [1280, 720, 100, 60],
    ] as const) {
      await page.setViewportSize({ width, height })
      // The narrowest creature of each board, and whether any board scrolls inside itself.
      const look = () =>
        page.evaluate(() => {
          const narrowest = (seat: number) =>
            Math.min(
              ...Array.from(
                document.querySelectorAll(
                  `[data-battlefield-seat="${seat}"] [data-lane="creatures"] button[data-object-id]`,
                ),
                // In whole px: a card of 190 px may measure 189.9999 after a resize.
              ).map((card) => Math.round(card.getBoundingClientRect().width)),
            )
          const scrolls = Array.from(document.querySelectorAll('[data-slot="battlefield-zone"]')).some(
            (zone) => zone.scrollHeight > zone.clientHeight + 1,
          )
          return { own: narrowest(0), theirs: narrowest(1), scrolls }
        })
      await expect
        .poll(look, { message: `${width}x${height}` })
        .toEqual({ own: expect.any(Number), theirs: expect.any(Number), scrolls: false })
      const seen = await look()
      expect(seen.own, `${width}x${height}: your creatures`).toBeGreaterThanOrEqual(own)
      expect(seen.theirs, `${width}x${height}: the opponent's creatures`).toBeGreaterThanOrEqual(theirs)
    }
  })

  test('fits at every size with three targeting items on the stack at rest', async ({ page }) => {
    await page.goto(`${DEV_WEB_URL}/dev/table?board=duel&prompt=priority&turn=you`)
    await page.locator(THINGS.primary).waitFor()
    // The band keeps the stack chips to a fifth of its width (docs/ui-design.md, The band): the chips on show and the
    // "+N more" button count the three items.
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            document.querySelectorAll('[data-stack-chip]').length +
            Number(document.querySelector('[data-slot="stack-more"]')?.textContent?.match(/\d+/)?.[0] ?? 0),
        ),
      )
      .toBe(3)
    await atEverySize(page, ['hand', 'life', 'primary'], onTable('three stack items'))
  })

  test('draws each seat’s corner as pictures of 44 px or more, and the opponent’s pod column does not scroll at 1920x1080, 1440x900 and 1366x768 (chips there)', async ({
    page,
  }) => {
    await page.goto(`${DEV_WEB_URL}/dev/table?board=duel&prompt=priority&turn=you`)
    await page.locator(THINGS.primary).waitFor()
    // The pod column of a seat: whether it scrolls, how its corner is drawn and the pictures in it.
    const read = () =>
      page.evaluate(() => {
        const read = (seat: number) => {
          const column = document.querySelector(`[data-slot="pod-column"]:has([data-seat="${seat}"])`)
          if (column === null) return null
          const box = column.getBoundingClientRect()
          const corner = column.querySelector('[data-slot="seat-corner"]')
          const pictures =
            corner === null
              ? []
              : [...corner.querySelectorAll('button[data-zone], [data-slot="corner-commander"]')]
          const rects = pictures.map((picture) => picture.getBoundingClientRect())
          return {
            scrolls: column.scrollHeight > column.clientHeight + 1,
            corner: corner !== null,
            pictures: pictures.length,
            narrowest: rects.length === 0 ? null : Math.min(...rects.map((rect) => Math.round(rect.width))),
            lowest: rects.length === 0 ? null : Math.min(...rects.map((rect) => Math.round(rect.height))),
            inside: rects.every(
              (rect) =>
                rect.left >= box.left - 0.5 &&
                rect.right <= box.right + 0.5 &&
                rect.bottom <= box.bottom + 0.5,
            ),
          }
        }
        return { opponent: read(1), you: read(0) }
      })
    // At 1366x768 the pictures would draw the opponent's creatures wider than yours: the pod keeps its chips and does not
    // scroll (opponent boards shrink first).
    await page.setViewportSize({ width: 1366, height: 768 })
    await expect
      .poll(async () => (await read()).opponent, { message: '1366x768: the opponent pod' })
      .toMatchObject({ scrolls: false })
    for (const [width, height] of [
      [1920, 1080],
      [1440, 900],
    ] as const) {
      await page.setViewportSize({ width, height })
      // The board split settles once the prompt sheet has told it the room it wants over the opponents' row.
      await expect
        .poll(async () => (await read()).opponent, {
          message: `${width}x${height}: the opponent's corner`,
        })
        .toMatchObject({ corner: true, scrolls: false, pictures: 3 })
      // Your row settles with the opponents': both are read once neither moves.
      await expect
        .poll(async () => (await read()).you, { message: `${width}x${height}: your pod settles` })
        .toMatchObject({ scrolls: false })
      const { opponent, you } = await read()
      // The commander, the graveyard and the exile of the opponent; the graveyard and the exile of yours.
      expect(opponent, `${width}x${height}: the opponent's pod`).toMatchObject({
        scrolls: false,
        inside: true,
        pictures: 3,
      })
      expect(opponent?.narrowest, `${width}x${height}: the opponent's pictures`).toBeGreaterThanOrEqual(44)
      expect(opponent?.lowest, `${width}x${height}: the opponent's pictures`).toBeGreaterThanOrEqual(62)
      expect(you, `${width}x${height}: your pod`).toMatchObject({ scrolls: false, inside: true, pictures: 2 })
      expect(you?.narrowest, `${width}x${height}: your pictures`).toBeGreaterThanOrEqual(44)
    }
    // Where the column is too short for the least picture, the chips are back and nothing is drawn under 44 px.
    for (const size of SIZES) {
      await page.setViewportSize({ width: size.width, height: size.height })
      await page.waitForTimeout(250)
      const { opponent } = await read()
      if (opponent === null || !opponent.corner) continue
      expect(
        opponent.narrowest,
        `${size.width}x${size.height}: the opponent's pictures`,
      ).toBeGreaterThanOrEqual(44)
      expect(opponent.inside, `${size.width}x${size.height}: the pictures are inside the column`).toBe(true)
    }
  })

  // The scene of five attachments on one creature, an opponent's Aura on yours, yours on theirs, a Curse and an Equipment on nothing.
  for (const [name, query] of [
    ['four seats', 'equip=1&prompt=priority&turn=you'],
    ['the duel board', 'board=duel&equip=1&prompt=priority&turn=you'],
  ] as const) {
    test(`fits at every size with the attachments scene on ${name}`, async ({ page }) => {
      await page.goto(`${DEV_WEB_URL}/dev/table?${query}`)
      await page.locator(THINGS.primary).waitFor()
      await atEverySize(page, ['hand', 'life', 'primary'], onTable(`the attachments scene on ${name}`))
    })
  }

  test('folds a crowded stack into counted chips inside the strip and bundles a big attack under whole labels, at every size', async ({
    page,
  }) => {
    // The chips on show, the "+N more" button and the bundle labels, against the strip and the window, read in one step.
    const read = () =>
      page.evaluate(() => {
        const sheet = document.querySelector('[data-slot="shared-sheet"]')?.getBoundingClientRect()
        const shown = [...document.querySelectorAll('[data-slot="stack-chips"] li:not([aria-hidden])')]
        const more = document.querySelector('[data-slot="stack-more"]')
        const outside = (box: DOMRect): boolean =>
          sheet === undefined ||
          box.left < sheet.left - 0.5 ||
          box.right > sheet.right + 0.5 ||
          box.top < sheet.top - 0.5 ||
          box.bottom > sheet.bottom + 0.5
        const labels = [...document.querySelectorAll('[data-bundle-label]')].map((label) => {
          const box = label.getBoundingClientRect()
          return {
            text: label.textContent ?? '',
            font: parseFloat(getComputedStyle(label).fontSize),
            out: box.left < 0 || box.top < 0 || box.right > innerWidth || box.bottom > innerHeight,
            onStrip:
              sheet !== undefined &&
              box.left < sheet.right &&
              sheet.left < box.right &&
              box.top < sheet.bottom &&
              sheet.top < box.bottom,
          }
        })
        return {
          shown: shown.length,
          counts: shown.flatMap((chip) => chip.querySelector('[data-slot="chip-count"]')?.textContent ?? []),
          outside: [...shown, ...(more === null ? [] : [more])].filter((chip) =>
            outside(chip.getBoundingClientRect()),
          ).length,
          top:
            document.querySelector('[data-slot="stack-chips"] li:not([aria-hidden]) [data-stack-chip]') !==
            null,
          more: more?.textContent ?? null,
          labels,
        }
      })
    await page.goto(`${DEV_WEB_URL}/dev/table?crowd=1&prompt=priority`)
    await page.locator('[data-bundle-label]').first().waitFor()
    await atEverySize(page, ['hand', 'life', 'primary'], onTable('the crowded scene'))
    for (const size of SIZES) {
      await page.setViewportSize({ width: size.width, height: size.height })
      await page.waitForTimeout(300)
      const seen = await read()
      const label = `${size.width}x${size.height}`
      expect(seen.top, `${label}: the top chip is on show`).toBe(true)
      expect(seen.outside, `${label}: chips outside the strip`).toBe(0)
      // Sixteen items: the folded chips and the "+N more" button account for every one.
      const hidden = Number(seen.more?.match(/\d+/)?.[0] ?? 0)
      const folded = seen.counts.reduce((sum, count) => sum + Number(count.slice(1)) - 1, 0)
      expect(seen.shown + folded + hidden, `${label}: every item counted once`).toBe(16)
      // The runs fold in order from the top, as many as the band's fifth for the chips shows.
      expect(['×2', '×4', '×6'].slice(0, seen.counts.length), `${label}: the runs fold`).toEqual(seen.counts)
      // Twelve attackers at three seats: one bundle each, its label whole, in the window and off the strip.
      expect(seen.labels.map((one) => one.text).toSorted(), `${label}: bundle labels`).toEqual([
        '4 attackers → Cy',
        '4 attackers → Dee',
        '4 attackers → you',
      ])
      for (const one of seen.labels) {
        expect(one.font, `${label}: ${one.text} under 12 px`).toBeGreaterThanOrEqual(12)
        expect(one.out, `${label}: ${one.text} leaves the window`).toBe(false)
        expect(one.onStrip, `${label}: ${one.text} lies on the strip`).toBe(false)
      }
    }
  })

  // The Commands panel open at every size (the rail's panel and the sheet included) with its tallest row, the look-at-the-top list, and with a picked card: the hand, your life and the pass button stay in reach.
  for (const panel of ['top', 'card'] as const) {
    test(`fits at every size with the Commands panel's ${panel} scene`, async ({ page }) => {
      await page.goto(`${DEV_WEB_URL}/dev/table?panel=${panel}`)
      await page.locator(THINGS.primary).waitFor()
      await page.locator('[data-slot="commands-panel"] [data-open]').waitFor()
      await atEverySize(page, ['hand', 'life', 'primary'], onTable(`the Commands panel's ${panel} scene`))
    })
  }

  test('writes the grant marks in whole words and the opponent’s corner as whole pictures or whole chips, at every size', async ({
    page,
  }) => {
    // What a page shows of its marks and of the cards in its pod columns, read in one step.
    const read = () =>
      page.evaluate(() => {
        const marks = [
          ...document.querySelectorAll('[data-slot="card-grant"], [data-slot="card-grants-more"]'),
        ].map((mark) => ({
          text: mark.textContent ?? '',
          // A line that the box cuts has more text than room (`truncate`): its word is cut.
          cut: [...mark.querySelectorAll('.truncate')].some(
            (line) => line.scrollWidth > line.clientWidth + 0.5,
          ),
          font: parseFloat(getComputedStyle(mark).fontSize),
        }))
        const columns = [...document.querySelectorAll('[data-slot="pod-column"]')].map((column) => {
          const box = column.getBoundingClientRect()
          const cards = [...column.querySelectorAll('[data-object-id]')]
          return {
            picture: column.querySelector('[data-slot="seat-corner"]') !== null,
            cards: cards.length,
            outside: cards.filter((card) => {
              const rect = card.getBoundingClientRect()
              return rect.top < box.top - 0.5 || rect.bottom > box.bottom + 0.5
            }).length,
          }
        })
        return { marks, columns }
      })
    await page.goto(`${DEV_WEB_URL}/dev/table?board=duel&equip=1`)
    await page.locator('[data-slot="card-grant"]').first().waitFor()
    for (const size of SIZES) {
      await page.setViewportSize({ width: size.width, height: size.height })
      await page.waitForTimeout(300)
      const { marks, columns } = await read()
      const label = `${size.width}x${size.height}`
      expect(
        marks.filter((mark) => mark.cut).map((mark) => mark.text),
        `${label}: marks cut by their box`,
      ).toEqual([])
      expect(
        marks.filter((mark) => mark.font < 12).map((mark) => mark.text),
        `${label}: marks under 12 px`,
      ).toEqual([])
      // A corner of pictures keeps every card inside its column; a corner of chips draws no card at all.
      for (const column of columns) {
        expect(column.outside, `${label}: a card of a pod column leaves it`).toBe(0)
        if (!column.picture) expect(column.cards, `${label}: a column of chips has no card`).toBe(0)
      }
    }
  })

  // The viewer holds seven cards and a pair of partners in the command zone: nine cards of 68 px want 670 px of the hand dock.
  test('keeps an opening hand and two commanders in one row at every size above a phone, the controls dock as wide as its words need', async ({
    page,
  }) => {
    const read = () =>
      page.evaluate(() => {
        const slots = [...document.querySelectorAll('[data-region="hand"] [data-slot="hand-slot"]')]
        const dock = document.querySelector('[data-region="controls"]')
        return {
          cards: slots.length,
          rows: new Set(slots.map((slot) => Math.round(slot.getBoundingClientRect().top))).size,
          dock: Math.round(dock?.getBoundingClientRect().width ?? 0),
          hints: dock?.querySelectorAll('[data-slot="key-cap"]').length ?? 0,
          primary: dock?.querySelector('[data-slot="primary-action"]')?.textContent ?? '',
          scrolls: dock === null || dock === undefined ? false : dock.scrollWidth > dock.clientWidth + 1,
          main: document.querySelector('[data-slot="table-main"]')?.getBoundingClientRect().width ?? 0,
        }
      })
    // A spell on the stack (the longest words of the full dock) and the longest step to name in the short one.
    for (const query of ['prompt=priority&turn=you', 'prompt=priority-main&turn=you&step=begincombat']) {
      await page.goto(`${DEV_WEB_URL}/dev/table?${query}`)
      await page.locator(THINGS.primary).waitFor()
      for (const size of SIZES.filter((size) => size.breakpoint !== 'xs')) {
        await page.setViewportSize({ width: size.width, height: size.height })
        const label = `${query} at ${size.width}x${size.height}`
        await expect.poll(async () => (await read()).rows, { message: `${label}: the hand's rows` }).toBe(1)
        const seen = await read()
        expect(seen.cards, `${label}: the cards of the hand and the commanders`).toBe(9)
        expect(seen.scrolls, `${label}: the dock scrolls sideways`).toBe(false)
        // Key hints and the name of a spell only where the dock leaves the row its room (a main area of 1161 px, beside
        // the side column, `DOCK_FULL_MIN_MAIN_PX`).
        const roomy = seen.main >= 1161
        expect(seen.hints > 0, `${label}: key hints`).toBe(roomy)
        if (!roomy) expect(seen.dock, `${label}: the dock`).toBeLessThanOrEqual(300)
        if (query.startsWith('prompt=priority&') && !roomy) expect(seen.primary).toBe('Resolve')
      }
    }
  })

  test('puts the commander tax in the row of the opponent’s zone chips, so its pod column does not scroll at 1440x900 and 1366x768', async ({
    page,
  }) => {
    const read = () =>
      page.evaluate(() => {
        const column = document.querySelector('[data-slot="pod-column"]:has([data-seat="1"])')
        return {
          found: column !== null,
          scrolls: column === null ? null : column.scrollHeight > column.clientHeight + 1,
          chips: column?.querySelector('[data-slot="corner-chips"]') != null,
          // The tax as a chip of the one row of chips, and none in a row of its own under it.
          taxInRow: column?.querySelector('[data-slot="corner-chips"] [data-slot="commander-tax"]') != null,
          taxRow: column?.querySelector('[data-slot="commander-chips"]') != null,
        }
      })
    // The scene with a Curse (a chip row of the pod's top) and a commander that was cast twice.
    await page.goto(`${DEV_WEB_URL}/dev/table?board=duel&equip=1`)
    await page.locator('[data-slot="card-grant"]').first().waitFor()
    // A phone has no pod column: its pods are one line over the board.
    for (const size of SIZES.filter((size) => size.breakpoint !== 'xs')) {
      await page.setViewportSize({ width: size.width, height: size.height })
      await expect
        .poll(async () => (await read()).found, { message: `${size.width}x${size.height}: the pod column` })
        .toBe(true)
      await page.waitForTimeout(250)
      const seen = await read()
      const label = `${size.width}x${size.height}`
      expect(seen.taxRow, `${label}: the tax has no row of its own`).toBe(false)
      if (seen.chips) expect(seen.taxInRow, `${label}: the tax is a chip of the zone chips' row`).toBe(true)
      // The column that holds the pod's blocks and one row of chips at the least height (150 px) and does not scroll.
      if (size.width === 1440 || size.width === 1366) {
        expect(seen.scrolls, `${label}: the opponent's pod column scrolls`).toBe(false)
      }
    }
  })

  test('writes a keyword wider than the foot stack of a picture in a short form its mark holds, at every size', async ({
    page,
  }) => {
    // The card search names every card of the scene and each picture is a plain card back, so the faces are pictures with the marks over them.
    await page.route(/\/api\/cards\?/, async (route) => {
      const name = new URL(route.request().url()).searchParams.get('q') ?? ''
      await route.fulfill({
        json: {
          cards: [
            {
              oracle_id: `dev-${name.toLowerCase().replace(/[^a-z]+/g, '-')}`,
              name,
              mana_cost: '',
              mana_value: 0,
              type_line: 'Creature',
              support: 'verified',
              colors: [],
              color_identity: [],
              images: {},
            },
          ],
          total: 1,
          page: 1,
          per_page: 20,
        },
      })
    })
    await page.route(/\/api\/images\//, (route) =>
      route.fulfill({
        contentType: 'image/svg+xml',
        body: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 5 7"><rect width="5" height="7" fill="#556"/></svg>',
      }),
    )
    const read = () =>
      page.evaluate(() =>
        [...document.querySelectorAll('[data-slot="card-overlay"] [data-slot="card-grant"]')].map((mark) => ({
          text: mark.textContent ?? '',
          card: mark.closest('[data-object-id]')?.getBoundingClientRect().width ?? 0,
          // A line that the box cuts has more text than room (`truncate`): its word ends in an ellipsis.
          cut: [...mark.querySelectorAll('.truncate')].some(
            (line) => line.scrollWidth > line.clientWidth + 0.5,
          ),
        })),
      )
    // The marks lie over pictures only once every card's search and picture have come back, one routed request each,
    // which a busy machine serves slowly: the wait is for that state, bounded by the test and not by a guess.
    test.setTimeout(120_000)
    await page.goto(`${DEV_WEB_URL}/dev/table?board=duel&equip=1&cards=real`)
    await page
      .locator('[data-slot="card-overlay"] [data-slot="card-grant"]')
      .first()
      .waitFor({ timeout: 60_000 })
    // The table fits itself over a few frames after each resize: two equal reads in a row are the settled marks.
    const settled = async (label: string) => {
      let last = ''
      await expect
        .poll(
          async () => {
            const now = JSON.stringify(await read())
            const same = now === last
            last = now
            return same
          },
          { message: `${label}: the marks settle`, intervals: [150], timeout: 30_000 },
        )
        .toBe(true)
      return read()
    }
    let short = 0
    let whole = 0
    for (const size of SIZES) {
      await page.setViewportSize({ width: size.width, height: size.height })
      const label = `${size.width}x${size.height}`
      const marks = await settled(label)
      expect(
        marks.filter((mark) => mark.cut).map((mark) => mark.text),
        `${label}: marks cut`,
      ).toEqual([])
      // "Deathtouch" is 73 px with its padding: whole in a foot stack (62 % of the card) of 74 px, else shortened.
      for (const mark of marks.filter((mark) => mark.text.startsWith('Death'))) {
        const fits = mark.card * 0.62 >= 74.2
        expect(mark.text, `${label}: Deathtouch on a card of ${Math.round(mark.card)} px`).toBe(
          fits ? 'Deathtouch' : 'Deatht.',
        )
        if (fits) whole += 1
        else short += 1
      }
    }
    // The scene shows the keyword in both forms, so the check is not about nothing.
    expect(short, 'Deathtouch in its short form').toBeGreaterThan(0)
    expect(whole, 'Deathtouch whole').toBeGreaterThan(0)
  })

  test('fits with a table of twelve seats in focus mode', async ({ page }) => {
    await page.goto(`${DEV_WEB_URL}/dev/table?seats=12&prompt=priority&turn=you`)
    await page.locator(THINGS.primary).waitFor()
    await atEverySize(page, ['hand', 'life', 'primary'], onTable('12 seats'))
  })

  for (const theme of ['light', 'dark'] as const) {
    for (const seats of [16, 24, 40]) {
      test(`fits at every size with ${seats} seats in the ${theme} theme`, async ({ browser }, testInfo) => {
        // Twenty sizes of a table with up to 40 seats of 40 permanents are laid out one after the other.
        test.setTimeout(300_000)
        const context = await themedContext(browser, theme)
        const page = await context.newPage()
        await page.goto(`${DEV_WEB_URL}/dev/table?seats=${seats}&prompt=priority&turn=you`)
        await page.locator(THINGS.primary).waitFor()
        await expectTheme(page, theme)
        await atEverySize(page, ['hand', 'life', 'primary'], onTable(`${seats} seats, ${theme}`))
        for (const size of [SIZES[1], SIZES[5]]) {
          if (size === undefined) continue
          await page.setViewportSize(size)
          await expectFits(page, size, ['hand', 'life', 'primary'], onTable(`${seats} seats, ${theme}`))
          await testInfo.attach(`table-${seats}-${theme}-${size.width}.png`, {
            body: await page.screenshot(),
            contentType: 'image/png',
          })
        }
        await context.close()
      })
    }
  }

  for (const [name, size] of [
    ['a phone', { width: 390, height: 844 }],
    ['a short phone', { width: 390, height: 640 }],
    ['a tablet', { width: 640, height: 768 }],
    ['a rail', { width: 1100, height: 800 }],
  ] as const) {
    test(`keeps the block prompt pressable under the Commands panel of ${name}`, async ({ page }) => {
      await page.setViewportSize(size)
      await page.goto(`${DEV_WEB_URL}/dev/table?seats=2&prompt=block&turn=you`)
      const block = page.getByRole('button', { name: 'No blocks' })
      await block.waitFor()
      const sheet = page.getByRole('button', { name: /^Commands, log, chat and stack/ })
      // The development effects panel lies over the top bar's button, so the press is sent to the button itself.
      if (await sheet.isVisible()) await sheet.evaluate((button) => (button as HTMLElement).click())
      // The side opens on the log; Commands is one press away.
      await page.getByRole('tab', { name: 'Commands' }).click()
      await expect(page.getByRole('region', { name: 'Commands' })).toBeVisible()
      await expect(page.locator('[data-slot="side-overlay"][data-lowered]')).toBeAttached()
      // A trial click fails when another element would take the press.
      await block.click({ trial: true })
      // The band's line is `display: contents` (its parts are cells of the band's grid), so its words are what show: the
      // sentence, or a combat prompt's line in its place.
      const sentence = page
        .locator('[data-slot="decision-line"] [data-slot="prompt-text"]')
        .filter({ visible: true })
        .first()
      await expect(sentence).toBeVisible()
      const covered = await sentence.evaluate((line) => {
        const box = line.getBoundingClientRect()
        const hit = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2)
        return hit === null || !line.contains(hit)
      })
      expect(covered, 'the prompt line is under the panel').toBe(false)
      // The panel keeps its tabs in view.
      await expect(page.getByRole('tab', { name: 'Log' })).toBeInViewport()
    })
  }
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

test.describe('every mode with bare boards', () => {
  // The opening hands and the first turn come before any permanent (`bare=1`). Off a phone the opening hand stands centred
  // over the table between the top bar and the dock, whatever the opponents' rows draw (docs/web.md, Opening hands);
  // an empty opponents' row keeps its least share and never outgrows yours (Board split, Boss mode).
  for (const mode of TABLE_MODES) {
    test(`${mode.name}: the opening hand stands centred over the table, whole, at every size`, async ({
      page,
    }) => {
      await page.goto(`${DEV_WEB_URL}/dev/table?${mode.query}&prompt=mulligan&bare=1`)
      await page.locator(THINGS.keep).first().waitFor()
      await atEverySize(
        page,
        ['hand', 'life', 'keep'],
        onTable(`${mode.name} at the opening hands`),
        (size) => expectOpening(page, size, `the opening hand of ${mode.name}`),
      )
    })

    test(`${mode.name}: the empty opponents' row keeps its share and never outgrows yours, at every size`, async ({
      page,
    }) => {
      await page.goto(`${DEV_WEB_URL}/dev/table?${mode.query}&prompt=priority&turn=you&bare=1`)
      await page.locator(THINGS.primary).waitFor()
      await atEverySize(
        page,
        ['hand', 'life', 'primary'],
        onTable(`${mode.name} on an empty first turn`),
        (size) =>
          expect
            .poll(
              async () => {
                const rows = await boardRows(page)
                if (rows.phone) return []
                const problems: string[] = []
                const room = rows.opponents + rows.own
                if (rows.opponents < room * EMPTY_ROW_LEAST_SHARE - 2) {
                  problems.push(
                    `the opponents' row is ${Math.round(rows.opponents)} of ${Math.round(room)} px`,
                  )
                }
                if (rows.opponents > rows.own + 1) problems.push(`the opponents' row outgrows yours`)
                return problems
              },
              { message: `${mode.name} at ${size.width}x${size.height}`, timeout: 5_000 },
            )
            .toEqual([]),
      )
    })
  }
})

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
    await atEverySize(
      deciding,
      ['hand', 'life', 'keep'],
      onTable('the opening hands of a real game'),
      (size) => expectOpening(deciding, size, 'the opening hand of a real game'),
    )
    // The sweep resized that page in place, so the seat keeps its hand on a page of one size.
    const swept = deciding === ada.page ? ada : bo
    await reseat(swept)
    await expect(swept.page.locator('[data-decision="Mulligan"]')).toBeVisible()

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
    // A look back: the result closes for the final table and its log, read only, and comes back. The sweep resized the
    // page in place, so the look back clicks on a new page of the seat.
    await reseat(ada, { width: 1440, height: 900 })
    const page = ada.page
    await page.locator('[data-slot="review-open"]').click()
    const strip = page.locator('[data-slot="review-bar"]')
    await expect(strip).toBeVisible()
    await expect(page.getByRole('region', { name: 'Game table' })).toBeVisible()
    await expect(strip.getByRole('link', { name: 'Back to Play' })).toBeVisible()
    const strays = await page.evaluate(() => {
      const { top, bottom, left, right } =
        document.querySelector('[data-slot="review-bar"]')?.getBoundingClientRect() ?? new DOMRect()
      return {
        inside: top >= 0 && left >= 0 && right <= innerWidth && bottom <= innerHeight,
        page: document.documentElement.scrollHeight > innerHeight,
      }
    })
    expect(strays).toEqual({ inside: true, page: false })
    // The Log tab lists what happened, and no decision waits.
    await page.getByRole('tab', { name: /^Log/ }).click()
    await expect(page.getByRole('log').first()).toBeVisible()
    await expect(page.locator('[data-decision]')).toHaveCount(0)
    await strip.getByRole('button', { name: 'Show result' }).click()
    await expect(page.locator(THINGS.result)).toBeVisible()
    await expect(strip).toHaveCount(0)
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})

test.describe('the play-session findings at laptop and desktop sizes', () => {
  /** The sizes every finding is held at besides its own. */
  const LAPTOPS = [
    [1366, 768],
    [1920, 1080],
  ] as const
  const at = async (page: Page, query: string, width: number, height: number) => {
    await page.setViewportSize({ width, height })
    await page.goto(`${DEV_WEB_URL}/dev/table?${query}`)
    await page.locator('[data-area="hand"]').waitFor()
  }

  for (const [name, query] of [
    ['the duel', 'seats=2&board=duel&side=folded'],
    ['four seats', 'seats=4&side=folded'],
    ['the boss fight', 'boss=1&side=folded'],
  ] as const) {
    test(`keeps your board's lower right cards clear of the grown controls dock on ${name}`, async ({
      page,
    }) => {
      for (const [width, height] of [[1280, 720], [1400, 900], ...LAPTOPS] as const) {
        for (const prompt of ['targets-two', 'x', 'pay']) {
          await at(page, `${query}&prompt=${prompt}`, width, height)
          await page.locator('[data-slot="dock-body"] [data-slot="prompt-actions"]').first().waitFor()
          const covered = () =>
            page.evaluate(() => {
              const body = document.querySelector('[data-slot="dock-body"]')?.getBoundingClientRect()
              if (body === undefined) return ['no dock']
              return [...document.querySelectorAll('[data-area="own"] button[data-object-id]')]
                .filter((card) => {
                  const r = card.getBoundingClientRect()
                  // A card its board has scrolled out of view is not drawn there.
                  const zone = card.closest('[data-slot="battlefield-zone"]')?.getBoundingClientRect()
                  const shown = zone === undefined || (r.top < zone.bottom - 1 && r.bottom > zone.top + 1)
                  return (
                    shown &&
                    r.width > 0 &&
                    r.right > body.left + 1 &&
                    r.bottom > body.top + 1 &&
                    r.left < body.right
                  )
                })
                .map((card) => card.getAttribute('aria-label') ?? '')
            })
          await expect
            .poll(covered, { message: `${width}x${height} ${prompt}: cards under the dock` })
            .toEqual([])
        }
      }
    })
  }

  for (const [name, query] of [
    ['the duel', 'board=duel'],
    ['four seats', 'seats=4'],
    ['the boss fight', 'seats=2&boss=0&bare=1'],
  ] as const) {
    test(`leaves the hand two whole cards beside a Satchel of twelve under priority on ${name}`, async ({
      page,
    }) => {
      for (const [width, height] of [[1280, 720], [1400, 900], ...LAPTOPS] as const) {
        await at(page, `${query}&prompt=priority&turn=you&satchel=12`, width, height)
        const whole = () =>
          page.evaluate(() => {
            const line = document.querySelector('[data-region="hand"] ul[aria-label$="\'s hand"]')
            if (line === null) return 0
            const box = line.getBoundingClientRect()
            return [...line.querySelectorAll('button[data-object-id]')].filter((card) => {
              const r = card.getBoundingClientRect()
              return r.left >= box.left - 1 && r.right <= box.right + 1 && r.right <= window.innerWidth + 1
            }).length
          })
        await expect
          .poll(whole, { message: `${width}x${height}: whole hand cards` })
          .toBeGreaterThanOrEqual(2)
      }
    })
  }

  /** Every prompt card: its width, the face it draws and whether a line of the face runs past its box. */
  const promptCards = (page: Page) =>
    page.evaluate(() =>
      [...document.querySelectorAll<HTMLElement>('[data-candidate]')]
        .filter((card) => card.getBoundingClientRect().width > 0)
        .map((card) => {
          const face = card.querySelector('[data-slot="card-face"]')?.getBoundingClientRect()
          const cut =
            face !== undefined &&
            [...card.querySelectorAll('[data-slot="card-face"] *')].some((part) => {
              if (part.children.length > 0 || (part.textContent ?? '').trim() === '') return false
              const r = part.getBoundingClientRect()
              return r.width > 0 && (r.bottom > face.bottom + 0.5 || r.right > face.right + 0.5)
            })
          return { width: card.getBoundingClientRect().width, face: card.dataset['face'], cut }
        }),
    )

  test('draws prompt cards under 76 px as the compact face, whole', async ({ page }) => {
    for (const [width, height] of [[1280, 720], [390, 844], ...LAPTOPS] as const) {
      for (const prompt of ['cards-exile', 'targets-zones']) {
        await at(page, `seats=4&prompt=${prompt}`, width, height)
        await expect.poll(async () => (await promptCards(page)).length).toBeGreaterThan(0)
        for (const card of await promptCards(page)) {
          const label = `${width}x${height} ${prompt}: a card ${Math.round(card.width)} px wide`
          if (card.width < 76) expect(card.face, label).toBe('compact')
          expect(card.cut, `${label} cuts a line of its face`).toBe(false)
        }
      }
    }
  })

  /** The prompt's cards: how many, the first row and how much of it is whole in every box that clips it, and the rows. */
  const firstRow = (page: Page) =>
    page.evaluate(() => {
      const cards = [...document.querySelectorAll('[data-candidate]')].filter(
        (card) => card.getBoundingClientRect().width > 0,
      )
      const top = Math.min(...cards.map((card) => card.getBoundingClientRect().top))
      const row = cards.filter((card) => Math.abs(card.getBoundingClientRect().top - top) < 4)
      const whole = (card: Element): boolean => {
        const r = card.getBoundingClientRect()
        let [low, high, left, right] = [0, window.innerHeight, 0, window.innerWidth]
        for (let box = card.parentElement; box !== null; box = box.parentElement) {
          const style = getComputedStyle(box)
          if (!/auto|scroll|hidden|clip/.test(style.overflowX + style.overflowY)) continue
          const clip = box.getBoundingClientRect()
          low = Math.max(low, clip.top)
          high = Math.min(high, clip.bottom)
          left = Math.max(left, clip.left)
          right = Math.min(right, clip.right)
        }
        return r.top >= low - 1 && r.bottom <= high + 1 && r.left >= left - 1 && r.right <= right + 1
      }
      const rows = new Set(cards.map((card) => Math.round(card.getBoundingClientRect().top))).size
      return { cards: cards.length, row: row.length, whole: row.filter(whole).length, rows }
    })

  for (const [name, query] of [
    ['the Stack panel', 'seats=4'],
    ['the window over the table', 'seats=2&board=duel&side=folded'],
  ] as const) {
    test(`shows a card prompt's first row of cards whole without scrolling in ${name}, wrapping more than eight`, async ({
      page,
    }) => {
      for (const [width, height] of [[1280, 720], [1400, 900], ...LAPTOPS] as const) {
        for (const prompt of ['cards-exile', 'targets-zones', 'cards-graveyard']) {
          await at(page, `${query}&prompt=${prompt}`, width, height)
          await expect.poll(async () => (await firstRow(page)).cards).toBeGreaterThan(0)
          const seen = await firstRow(page)
          const label = `${width}x${height} ${prompt}`
          expect(seen.whole, `${label}: whole cards of the first row`).toBe(seen.row)
          await expect(
            page.locator('[data-slot="decision-line"]').first(),
            `${label}: the question`,
          ).toBeInViewport()
          if (seen.cards > 8) expect(seen.rows, `${label}: ${seen.cards} cards wrap`).toBeGreaterThan(1)
        }
      }
    })
  }

  test("shows a row of each opponent's cards in an eight-seat grid", async ({ page }) => {
    // An eight-seat grid at 1366x768 has no room for three rows of boards over yours: the table draws focus mode there.
    for (const [width, height] of [
      [1400, 900],
      [1920, 1080],
    ] as const) {
      await at(page, 'seats=8&mode=grid&prompt=priority', width, height)
      const rows = () =>
        page.evaluate(() =>
          [...document.querySelectorAll('[data-area="opponents"] [data-slot="seat-frame"]')].map((frame) => {
            const zone = frame.querySelector('[data-slot="battlefield-zone"]')?.getBoundingClientRect()
            if (zone === undefined) return 0
            return [...frame.querySelectorAll('button[data-object-id]')].filter((card) => {
              const r = card.getBoundingClientRect()
              return r.width > 0 && r.top >= zone.top - 1 && r.bottom <= zone.bottom + 1
            }).length
          }),
        )
      await expect.poll(async () => (await rows()).length).toBe(7)
      await expect
        .poll(async () => Math.min(...(await rows())), {
          message: `${width}x${height}: whole cards of a board`,
        })
        .toBeGreaterThanOrEqual(1)
    }
  })

  test('shows every chip of your pod column in boss mode without scrolling', async ({ page }) => {
    // With a party beside you the table has the room from 900 px high; without one, from 768.
    for (const [query, width, height] of [
      ['boss=1', 1400, 900],
      ['boss=1', 1920, 1080],
      ['boss=2', 1400, 900],
      ['boss=2', 1366, 768],
      ['boss=2', 1920, 1080],
    ] as const) {
      await at(page, `${query}&prompt=priority`, width, height)
      const scrolls = () =>
        page.evaluate(() => {
          const column = document.querySelector('[data-slot="pod-column"]:has([data-seat="0"])')
          if (column === null) return -1
          return [column, ...column.querySelectorAll('[data-slot="pod-top-scroll"], [data-slot="pod-more"]')]
            .map((box) => Math.max(box.scrollHeight - box.clientHeight, box.scrollWidth - box.clientWidth))
            .reduce((a, b) => Math.max(a, b), 0)
        })
      await expect.poll(scrolls, { message: `${query} at ${width}x${height}: your pod column` }).toBe(0)
    }
  })
})
