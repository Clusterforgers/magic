// Goal: a whole turn on the development table can be walked with the keyboard alone at a desktop and a phone size: Tab visits the bar, the boards, the hand and the passing controls in reading order and wraps without a trap, the step ladder is one tab stop that arrows move through and Enter sets a stop, H opens a card's by-hand list and Escape returns focus to that card even when the window is resized in between, and the by-hand mark and the "Not now" hint of a card that cannot be played keep WCAG AA contrast (4.5:1 for text, 3:1 for the mark's fill) in the light and the dark theme.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { expectTheme, themedContext } from './layoutGate'
import type { Theme } from './layoutAudit'
import { DEV_WEB_URL } from './ports'

const DESKTOP = { width: 1600, height: 1000 }
const PHONE = { width: 390, height: 844 }
const TURN = `${DEV_WEB_URL}/dev/table?seats=2&turn=you&prompt=priority&hand=7`
const STATES = `${DEV_WEB_URL}/dev/table?states=1&turn=you&prompt=priority&hand=7`

/** Where a focused element sits: the table region it is in, or its slot, for the reading order. */
interface Stop {
  region: string
  slot: string
  name: string
}

/** The element that has focus now, as a `Stop`. */
const focused = (page: Page): Promise<Stop> =>
  page.evaluate(() => {
    const el = document.activeElement
    if (el === null || el === document.body) return { region: 'body', slot: '', name: '' }
    const region =
      el.closest('[data-region]')?.getAttribute('data-region') ??
      (el.closest('header') === null ? (el.closest('main') === null ? 'page' : 'main') : 'bar')
    const name = el.getAttribute('aria-label') ?? el.textContent ?? ''
    return { region, slot: el.getAttribute('data-slot') ?? '', name: name.slice(0, 60) }
  })

/** Opens a scene and waits until the table has drawn its hand. */
async function open(page: Page, url: string): Promise<void> {
  await page.goto(url)
  await expect(page.locator('[data-region="hand"] button[data-object-id]').first()).toBeVisible({
    timeout: 90_000,
  })
}

/** Presses Tab until `done` holds for the focused element, at most `limit` times; fails with the last stop. */
async function tabTo(page: Page, done: (stop: Stop) => boolean, limit = 80): Promise<Stop> {
  let last: Stop = { region: '', slot: '', name: '' }
  for (let press = 0; press < limit; press++) {
    await page.keyboard.press('Tab')
    last = await focused(page)
    if (done(last)) return last
  }
  throw new Error(`Tab never reached the element; last stop ${JSON.stringify(last)}`)
}

const SIZES = [
  { label: 'desktop', size: DESKTOP },
  { label: 'phone', size: PHONE },
]

for (const { label, size } of SIZES) {
  test(`the keyboard walks a turn at ${label} size in reading order`, async ({ page }) => {
    await page.setViewportSize(size)
    await open(page, TURN)

    // One lap of Tab from the page's start: it ends back at the start, visits no element twice and keeps the order
    // bar, boards, hand, controls.
    const lap: Stop[] = []
    const seen = new Set<string>()
    for (let press = 0; press < 90; press++) {
      await page.keyboard.press('Tab')
      const stop = await focused(page)
      if (stop.region === 'body' || (press > 0 && stop.name.startsWith('Skip to content'))) break
      const key = `${stop.region}|${stop.slot}|${stop.name}`
      expect(seen.has(key), `visited twice: ${key}`).toBe(false)
      seen.add(key)
      lap.push(stop)
    }
    const first = (region: string) => lap.findIndex((stop) => stop.region === region)
    const last = (region: string) => lap.findLastIndex((stop) => stop.region === region)
    expect(lap.length).toBeGreaterThan(10)
    expect(lap.length).toBeLessThan(60)
    expect(lap[0]?.name).toContain('Skip to content')
    expect(first('bar')).toBeGreaterThanOrEqual(0)
    expect(first('hand')).toBeGreaterThan(last('bar'))
    expect(first('controls')).toBeGreaterThan(last('hand'))
    expect(lap.some((stop) => stop.slot === 'primary-action')).toBe(true)

    // The step ladder is one tab stop wherever it sits in the page.
    expect(lap.filter((stop) => stop.slot === 'stop')).toHaveLength(label === 'desktop' ? 1 : 0)

    // Shift+Tab walks the same stops backward: back from the last stop of the lap to the one before it.
    const lastStop = lap.at(-1)
    const before = lap.at(-2)
    if (lastStop === undefined || before === undefined) throw new Error('lap too short')
    await tabTo(page, (stop) => stop.name === lastStop.name && stop.region === lastStop.region)
    await page.keyboard.press('Shift+Tab')
    expect((await focused(page)).name).toBe(before.name)
  })
}

test('the step ladder is one tab stop; arrows move across its marks and Enter sets and clears a stop', async ({
  page,
}) => {
  await page.setViewportSize(DESKTOP)
  await open(page, TURN)
  const start = await tabTo(page, (stop) => stop.slot === 'stop')
  const lit = page.locator('[data-slot="stop"][aria-pressed="true"]')
  await page.keyboard.press('ArrowRight')
  const moved = await focused(page)
  expect(moved.slot).toBe('stop')
  expect(moved.name).not.toBe(start.name)
  await page.keyboard.press('End')
  const end = await focused(page)
  await page.keyboard.press('Home')
  const home = await focused(page)
  expect(end.name).not.toBe(home.name)
  await expect(lit).toHaveCount(0)
  await page.keyboard.press('Enter')
  await expect(lit).toHaveCount(1)
  expect((await focused(page)).name.startsWith(home.name.split(':')[0] ?? '')).toBe(true)
  await page.keyboard.press('Enter')
  await expect(lit).toHaveCount(0)
  // Tab leaves the ladder for the next control and Shift+Tab comes back to the mark that was last used.
  await page.keyboard.press('Tab')
  expect((await focused(page)).slot).not.toBe('stop')
  await page.keyboard.press('Shift+Tab')
  expect((await focused(page)).name.startsWith(home.name.split(':')[0] ?? '')).toBe(true)
})

test('the hand is one tab stop with arrows, Home and End', async ({ page }) => {
  await page.setViewportSize(DESKTOP)
  await open(page, TURN)
  const handCards = page.locator('[data-region="hand"] ul[aria-label$="\'s hand"] button[data-object-id]')
  const count = await handCards.count()
  expect(count).toBe(7)
  const entry = await tabTo(page, (stop) => stop.region === 'hand')
  // Tab may first reach the command zone list; the hand list is the next stop with seven cards.
  if (!(await handCards.first().evaluate((el) => el === document.activeElement))) {
    await tabTo(page, (stop) => stop.region === 'hand' && stop.name !== entry.name)
  }
  const names = new Set<string>()
  for (let index = 0; index < count; index++) {
    names.add((await focused(page)).name)
    await page.keyboard.press('ArrowRight')
  }
  expect(names.size).toBeGreaterThan(1)
  await page.keyboard.press('End')
  const end = await focused(page)
  await page.keyboard.press('Home')
  expect((await focused(page)).name).not.toBe(end.name)
  // One Tab leaves the whole row.
  await page.keyboard.press('Tab')
  expect((await focused(page)).region).not.toBe('hand')
})

/** Tabs to the board card named `name` (its accessible name starts with it) and returns its object id. */
async function focusCard(page: Page, name: string): Promise<string> {
  // A lane is one tab stop: Tab to the lane that holds the card, then arrow along it.
  const holds = (): Promise<boolean> =>
    page.evaluate(
      (card) => document.activeElement?.closest('ul')?.querySelector(`[aria-label^="${card}"]`) != null,
      name,
    )
  for (let press = 0; press < 120 && !(await holds()); press++) await page.keyboard.press('Tab')
  expect(await holds(), 'Tab reached the lane').toBe(true)
  for (let step = 0; step < 40 && !(await focused(page)).name.startsWith(name); step++) {
    await page.keyboard.press('ArrowRight')
  }
  expect((await focused(page)).name.startsWith(name)).toBe(true)
  const id = await page.evaluate(() => document.activeElement?.getAttribute('data-object-id') ?? '')
  expect(id).not.toBe('')
  return id
}

const RESIZES = [
  { label: 'desktop to phone', from: DESKTOP, to: PHONE },
  { label: 'phone to desktop', from: PHONE, to: DESKTOP },
]

for (const { label, from, to } of RESIZES) {
  test(`H opens the by-hand list and Escape returns focus to the card across a resize, ${label}`, async ({
    page,
  }) => {
    await page.setViewportSize(from)
    await open(page, STATES)
    const id = await focusCard(page, 'Wild Mage Apprentice')
    await page.keyboard.press('h')
    const dialog = page.getByRole('dialog', { name: /rules played by hand/ })
    await expect(dialog).toBeVisible()
    await expect(dialog).toContainText('coin')

    await page.setViewportSize(to)
    await expect(dialog).toBeVisible()
    // Focus stays inside the open list while the layout changes.
    await expect
      .poll(() => page.evaluate(() => document.activeElement?.closest('[role="dialog"]') !== null))
      .toBe(true)

    await page.keyboard.press('Escape')
    await expect(dialog).toBeHidden()
    await expect
      .poll(() => page.evaluate(() => document.activeElement?.getAttribute('data-object-id') ?? ''))
      .toBe(id)
  })
}

test('Escape after H with no resize also returns focus to the card', async ({ page }) => {
  await page.setViewportSize(DESKTOP)
  await open(page, STATES)
  const id = await focusCard(page, 'Wild Mage Apprentice')
  await page.keyboard.press('h')
  await expect(page.getByRole('dialog', { name: /rules played by hand/ })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect
    .poll(() => page.evaluate(() => document.activeElement?.getAttribute('data-object-id') ?? ''))
    .toBe(id)
})

/** The WCAG 2 contrast ratio of two sRGB colors given as [r, g, b] in 0..255. */
function ratio(a: number[], b: number[]): number {
  const lum = (c: number[]) => {
    const [r = 0, g = 0, bl = 0] = c.map((v) => {
      const s = v / 255
      return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
    })
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl
  }
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x)
  return ((hi ?? 0) + 0.05) / ((lo ?? 0) + 0.05)
}

/** What the page says of one mark: its text color, fill and surrounding colors as sRGB, read off computed styles. */
interface Paint {
  text: number[]
  fill: number[]
  around: number[]
  hasWords: boolean
}

/**
 * Reads the paint of the first element matching `selector`: its color, the opaque color under it (its own background
 * blended over its ancestors') and what surrounds it (its border when it has one, else the ancestors' background).
 */
function paintOf(page: Page, selector: string): Promise<Paint> {
  return page.evaluate((sel) => {
    const canvas = document.createElement('canvas')
    canvas.width = canvas.height = 1
    const ctx = canvas.getContext('2d', { willReadFrequently: true })
    if (ctx === null) throw new Error('no canvas')
    const rgba = (css: string): number[] => {
      ctx.clearRect(0, 0, 1, 1)
      ctx.fillStyle = '#000'
      ctx.fillStyle = css
      ctx.fillRect(0, 0, 1, 1)
      const [r = 0, g = 0, b = 0, a = 0] = ctx.getImageData(0, 0, 1, 1).data
      return [r, g, b, a / 255]
    }
    const over = (top: number[], under: number[]): number[] => {
      const a = top[3] ?? 1
      return [0, 1, 2].map((i) => (top[i] ?? 0) * a + (under[i] ?? 0) * (1 - a)).concat([1])
    }
    const el = document.querySelector<HTMLElement>(sel)
    if (el === null) throw new Error(`nothing matches ${sel}`)
    const chain: HTMLElement[] = []
    for (let up: HTMLElement | null = el; up !== null; up = up.parentElement) chain.push(up)
    const underOf = (from: number): number[] => {
      let paint = rgba(getComputedStyle(document.body).backgroundColor)
      if (paint[3] !== 1) paint = [255, 255, 255, 1]
      for (let i = chain.length - 1; i >= from; i--) {
        const node = chain[i]
        if (node === undefined) continue
        paint = over(rgba(getComputedStyle(node).backgroundColor), paint)
      }
      return paint
    }
    const style = getComputedStyle(el)
    const border = parseFloat(style.borderTopWidth) > 0 ? over(rgba(style.borderTopColor), underOf(1)) : null
    return {
      text: over(rgba(style.color), underOf(0)).slice(0, 3),
      fill: underOf(0).slice(0, 3),
      around: (border ?? underOf(1)).slice(0, 3),
      hasWords: (el.textContent ?? '').trim().length > 0,
    }
  }, selector)
}

for (const theme of ['light', 'dark'] as const satisfies readonly Theme[]) {
  test(`the by-hand mark and the unplayable hint keep AA contrast in the ${theme} theme`, async ({
    browser,
  }) => {
    const context = await themedContext(browser, theme, { viewport: DESKTOP })
    try {
      const page = await context.newPage()
      await open(page, STATES)
      await expectTheme(page, theme)

      // The by-hand mark: its words (or icon) against its fill, and its fill against what it sits on.
      const mark = page.locator('[data-slot="card-support"]').first()
      await expect(mark).toBeVisible()
      const by = await paintOf(page, '[data-slot="card-support"]')
      expect(ratio(by.text, by.fill), 'by-hand text on its fill').toBeGreaterThanOrEqual(
        by.hasWords ? 4.5 : 3,
      )
      expect(ratio(by.fill, by.around), 'by-hand fill against its surround').toBeGreaterThanOrEqual(3)

      // The hint of a card that cannot be played shows on focus: "Not now" on the card's foot.
      await page
        .locator('[data-region="hand"] button[data-object-id]:has([data-slot="card-hint"])')
        .first()
        .focus()
      await expect(page.locator('[data-region="hand"] [data-slot="card-hint"]:visible').first()).toBeVisible()
      const hint = await paintOf(page, '[data-region="hand"] [data-slot="card-hint"]:not([hidden])')
      expect(ratio(hint.text, hint.fill), 'unplayable hint text on its fill').toBeGreaterThanOrEqual(4.5)
    } finally {
      await context.close()
    }
  })
}
