// Goal: the site header of a guest, an account and the admin fits at 1920, 1440, 1280, 1024, 768, 390 and 360 px wide: nothing in it is cut off or scrolls sideways, Create, the theme and the account stay in the window, and every tab of the variant is either in the row or under More, whose menu fits too. Each width is a fresh context of its own, so no page is resized under a click.
import { expect, test } from './test'
import type { Browser, Page } from '@playwright/test'
import { measure, problemsOf, TEXT_FLOOR } from './layoutAudit'
import type { Ask } from './layoutAudit'
import { WEB_URL } from './ports'

const WIDTHS = [1920, 1440, 1280, 1024, 768, 390, 360] as const

const GUEST_TABS = ['Play', 'Precons']
const ACCOUNT_TABS = [
  'Play',
  'Precons',
  'Decks',
  'My games',
  'Tournaments',
  'Delves',
  'Review cards',
  'Card support',
]

/** A header variant: who is signed in, if anyone, and what the header must show. */
interface Variant {
  name: string
  /** The answer of `GET /api/auth/me`: an account, or null for a guest. */
  me: { id: string; username: string; is_admin: boolean } | null
  tabs: string[]
  /** The account control: the menu of an account, Sign in for a guest. */
  account: string
}

// The longest names break a header first.
const VARIANTS: Variant[] = [
  { name: 'signed out', me: null, tabs: GUEST_TABS, account: 'nav[aria-label="Account"] a[href="/login"]' },
  {
    name: 'signed in',
    me: { id: 'header-fit', username: 'long_layout_account_name', is_admin: false },
    tabs: ACCOUNT_TABS,
    account: 'nav[aria-label="Account"] button',
  },
  {
    name: 'admin',
    me: { id: 'header-fit-admin', username: 'long_layout_admin_name_x', is_admin: true },
    tabs: ACCOUNT_TABS,
    account: 'nav[aria-label="Account"] button',
  },
]

/** A page of its own at `width`, signed in as `variant` says (the server of this run has no accounts), at Play. */
async function headerAt(browser: Browser, variant: Variant, width: number): Promise<Page> {
  const context = await browser.newContext({ baseURL: WEB_URL, viewport: { width, height: 800 } })
  await context.route('**/api/auth/me', (route) =>
    variant.me === null
      ? route.fulfill({
          status: 401,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'unauthenticated', message: 'Sign in first.' }),
        })
      : route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(variant.me) }),
  )
  const page = await context.newPage()
  await page.goto('/')
  await page.locator(variant.account).first().waitFor()
  if (variant.me !== null)
    await page
      .locator('[data-measure="tab"]')
      .nth(ACCOUNT_TABS.length - 1)
      .waitFor({ state: 'attached' })
  await page.evaluate(() => document.fonts.ready.then(() => undefined))
  return page
}

/** The header's problems in the page as it stands, judged by the layout gate's rules. */
async function headerProblems(page: Page, variant: Variant): Promise<string[]> {
  const ask: Ask = {
    actions: {
      Create: 'header a[href="/new"]',
      Theme: 'header button[aria-label^="Theme"]',
      Account: variant.account,
    },
    reveal: false,
    exempt: [],
    floor: TEXT_FLOOR,
  }
  return problemsOf(await page.evaluate(measure, ask), { fits: false })
}

for (const variant of VARIANTS) {
  test(`the ${variant.name} header fits at every width and offers every tab`, async ({ browser }) => {
    test.setTimeout(120_000)
    const broken: string[] = []
    for (const width of WIDTHS) {
      const page = await headerAt(browser, variant, width)
      const nav = page.getByRole('navigation', { name: 'Main' })
      const more = nav.getByRole('button', { name: /^More pages/ })
      // The row settles once the ruler has measured the tabs in the page's own fonts.
      await expect.poll(() => headerProblems(page, variant), { timeout: 4_000 }).toEqual([])
      const row = (await nav.getByRole('link').allTextContents()).filter((name) => name !== 'Create')
      let menu: string[] = []
      if (await more.isVisible()) {
        await more.click()
        const items = page.getByRole('menuitem')
        await items.first().waitFor()
        menu = await items.allTextContents()
        const open = await headerProblems(page, variant)
        if (open.length > 0) broken.push(`${width} menu open: ${open.join('; ')}`)
        await page.keyboard.press('Escape')
        await expect(more).toBeFocused()
      }
      const names = [...row, ...menu].map((name) => name.replace(/\d+\+?$/, '').trim())
      if (names.join('|') !== variant.tabs.join('|')) broken.push(`${width}: tabs ${names.join(', ')}`)
      if (width >= 1280 && menu.length > 0) broken.push(`${width}: More holds ${menu.join(', ')}`)
      await page.context().close()
    }
    expect(broken).toEqual([])
  })
}

test('More opens from the keyboard and leads to a page it holds', async ({ browser }) => {
  const page = await headerAt(browser, VARIANTS[1] as Variant, 390)
  const more = page.getByRole('navigation', { name: 'Main' }).getByRole('button', { name: /^More pages/ })
  await more.focus()
  await page.keyboard.press('Enter')
  const delves = page.getByRole('menuitem', { name: 'Delves' })
  await expect(page.getByRole('menuitem').first()).toBeFocused()
  await delves.focus()
  await page.keyboard.press('Enter')
  await expect(page).toHaveURL(/\/delves$/)
  await expect(page.getByRole('menu')).toHaveCount(0)
  await expect(more).toHaveAccessibleName(/the current page is one of them/)
})
