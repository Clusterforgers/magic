// The run of the layout gate that `layout.spec.ts` and `accounts.spec.ts` share: a themed browser context and a page held to the rules at every size.
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { expect } from '@playwright/test'
import type { Browser, BrowserContext, BrowserContextOptions, Page, TestInfo } from '@playwright/test'
import { GATE_SIZES, measure, SIZES, settle, sizeName, storeTheme, TEXT_FLOOR } from './layoutAudit'
import type { Ask, Theme } from './layoutAudit'

/** What a page is held to besides the rules every page keeps. */
export interface Rules {
  /** From this width on the page fits the window and never scrolls down: 0 for every size, Infinity for none. */
  fitsFrom: number
  /** The primary actions of the page: a name for the message and a CSS selector (or `text:Words` of a button or link). */
  actions: Record<string, string>
  /** Regions whose contents are cut off sideways on purpose. */
  exempt?: string[]
  /** Puts the page in the state to judge after each resize (a pane that a switch shows at a narrow width). */
  afterResize?: (page: Page) => Promise<void>
}

/** Under the lg breakpoint the builder shows one pane and a switch picks it; from there up the switch is gone. */
export async function showPane(page: Page, pane: 'Search' | 'Deck'): Promise<void> {
  const choose = page
    .getByRole('group', { name: 'Show' })
    .getByRole('button', { name: new RegExp(`^${pane}`) })
  if (await choose.isVisible()) await choose.click()
}

/** Awaits two frames, so that a layout made in reaction to a resize has been drawn before the page is read. */
const frames = (): Promise<void> =>
  new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))

/** A context whose system asks for `theme` and whose stored choice is `theme`, the way a person who set it has it. */
export async function themedContext(
  browser: Browser,
  theme: Theme,
  options: BrowserContextOptions = {},
): Promise<BrowserContext> {
  const context = await browser.newContext({
    colorScheme: theme,
    viewport: SIZES[0] ?? { width: 1920, height: 1080 },
    ...options,
  })
  await context.addInitScript(storeTheme, theme)
  return context
}

/** The page shows `theme`: the class on the root element that every colour follows. */
export async function expectTheme(page: Page, theme: Theme): Promise<void> {
  await expect
    .poll(() => page.evaluate(() => document.documentElement.classList.contains('dark')))
    .toBe(theme === 'dark')
}

/** Holds the page to the rules at every gate size, down and back up, and fails with each size that breaks one. */
export async function holdsAtEverySize(page: Page, rules: Rules, label: string): Promise<void> {
  const broken: string[] = []
  for (const size of [...GATE_SIZES, ...GATE_SIZES.toReversed()]) {
    const fits = size.width >= rules.fitsFrom
    const ask: Ask = { actions: rules.actions, reveal: !fits, exempt: rules.exempt ?? [], floor: TEXT_FLOOR }
    await page.setViewportSize(size)
    await rules.afterResize?.(page)
    const problems = await settle(
      async () => {
        await page.evaluate(frames)
        return page.evaluate(measure, ask)
      },
      { fits },
    )
    const line = `${sizeName(size)}: ${problems.join('; ')}`
    if (problems.length > 0 && !broken.includes(line)) broken.push(line)
  }
  expect(broken, label).toEqual([])
}

/** The sizes a stop with `shots` is screenshotted at: a phone and a laptop. */
export const SHOT_SIZES = [
  { width: 390, height: 844 },
  { width: 1440, height: 900 },
] as const

/** Where the gate's screenshots are written besides the report; none are written while it is unset (the repo keeps no images). */
const SHOTS_DIR = process.env['E2E_SHOTS_DIR']

/** Screenshots the page at each of `SHOT_SIZES`, attached to the test and written to `E2E_SHOTS_DIR` when it is set. */
export async function screenshots(page: Page, rules: Rules, name: string, testInfo: TestInfo): Promise<void> {
  for (const size of SHOT_SIZES) {
    await page.setViewportSize(size)
    await rules.afterResize?.(page)
    await page.evaluate(frames)
    const file = `${name.replace(/\W+/g, '-').toLowerCase()}-${size.width}x${size.height}.png`
    const body = await page.screenshot({ fullPage: true })
    await testInfo.attach(file, { body, contentType: 'image/png' })
    if (SHOTS_DIR !== undefined && SHOTS_DIR !== '') {
      await mkdir(SHOTS_DIR, { recursive: true })
      await writeFile(join(SHOTS_DIR, file), body)
    }
  }
}
