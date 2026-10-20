// Goal: the layout gate. Every page that needs no account (Play, Create, a lobby, an invite link, sign-in, register, the game-over screen) and the development routes that stand for the pages that do (the table, a lobby, the builder) keep the layout rules in Chromium at the six sizes of docs/ui-design.md (1920x1080, 1440x900, 1366x768, 1280x720, 1024x768, 390x844), down and back up, in the light and the dark theme: the page never scrolls sideways, no element is cut off sideways by the window or a box that clips it, no text is under 11 px, every primary action is in the window and not covered, and a page meant to fit the window never scrolls down. Decks, My games and the builder with a real deck need Postgres and are held by accounts.spec.ts.
import { expect, test } from '@playwright/test'
import type { APIRequestContext, Browser, BrowserContext, Page } from '@playwright/test'
import { playToTheEnd, Run } from './driver'
import { measure, problemsOf, SIZES, settle, sizeName, storeTheme, TEXT_FLOOR, THEMES } from './layoutAudit'
import type { Ask, Theme } from './layoutAudit'
import { createAndJoin, everyoneReady } from './lobby'
import { DEV_WEB_URL, WEB_URL } from './ports'

/** What a page is held to besides the rules every page keeps. */
interface Rules {
  /** From this width on the page fits the window and never scrolls down: 0 for every size, Infinity for none. */
  fitsFrom: number
  /** The primary actions of the page: a name for the message and a CSS selector (or `text:Words` of a button or link). */
  actions: Record<string, string>
  /** Regions whose contents are cut off sideways on purpose. */
  exempt?: string[]
  /** Puts the page in the state to judge after each resize (a pane that a switch shows at a narrow width). */
  afterResize?: (page: Page) => Promise<void>
}

/** A page of the gate and the way to it. */
interface Stop extends Rules {
  name: string
  /** Opens the page in the context and waits until it is drawn. */
  open(page: Page, context: BrowserContext): Promise<void>
}

/**
 * What the table scrolls or clips on purpose: the art of a card is a picture of a card, and the strip of opponents, the
 * chips of the stack and the hand scroll inside their own box where the width is short (docs/ui-design.md, Table
 * layout).
 */
const TABLE_EXEMPT = [
  '[data-slot="card-face"]',
  '[data-slot="tile-strip"]',
  '[data-slot="stack-chips"] ol',
  '[data-slot="hand-fan"]',
]

const unique = (prefix: string): string => `${prefix}${Math.random().toString(36).slice(2, 7)}`

/** Makes a game as a script would: the host's seat is held and the others are open, for a public or an invite game. */
async function makeGame(
  request: APIRequestContext,
  host: string,
  visibility: 'public' | 'invite',
  seats: number,
): Promise<{ game: string; code: string; seats: { seat: number; token: string }[] }> {
  const made = await request.post('/api/games', {
    headers: { origin: WEB_URL },
    data: {
      format: 'normal',
      visibility,
      held: 1,
      seats: Array.from({ length: seats }, (_, index) => ({
        name: index === 0 ? host : `Open seat ${index}`,
        deck: index % 2 === 0 ? 'mono-green' : 'mono-red',
      })),
    },
  })
  expect(made.ok()).toBe(true)
  return (await made.json()) as { game: string; code: string; seats: { seat: number; token: string }[] }
}

/** The create form, filled in by a person who gives a name and takes the rest as it is. */
async function createFromForm(page: Page, seats: number): Promise<void> {
  await page.goto('/new')
  await expect(page.getByRole('heading', { name: 'Create a game' })).toBeVisible()
  await page.getByLabel('Seats').selectOption(String(seats))
  await page.getByLabel('Your name').fill(unique('Host'))
  await page.getByRole('button', { name: 'Create game' }).click()
  await expect(page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
}

/** A server without a database says it has no accounts; the sign-in pages are shown for one that does but has nobody in. */
async function signedOut(context: BrowserContext): Promise<void> {
  await context.route('**/api/auth/me', (route) =>
    route.fulfill({
      status: 401,
      contentType: 'application/json',
      body: JSON.stringify({ error: 'unauthenticated', message: 'Sign in first.' }),
    }),
  )
}

const STOPS: readonly Stop[] = [
  {
    name: 'Play, with open games',
    fitsFrom: 0,
    actions: {
      'Create game': 'main a[href="/new"]',
      'Join with a code': '#join-code + button',
      'Join an open game': 'main button[aria-label^="Join "]',
    },
    async open(page, context) {
      await makeGame(context.request, unique('Ada'), 'public', 2)
      await makeGame(context.request, 'Bartholomew-the-Long-Name-32', 'public', 4)
      await makeGame(context.request, unique('Cy'), 'public', 6)
      await page.goto('/')
      await expect(page.getByRole('heading', { level: 1, name: 'Play' })).toBeVisible()
      await expect(page.getByRole('button', { name: /^Join .+ seats?$/ }).first()).toBeVisible()
    },
  },
  {
    name: 'Create a game',
    // A form on a phone scrolls; its button is reached by scrolling.
    fitsFrom: 640,
    actions: { 'Create game': 'main form button[type="submit"]' },
    async open(page) {
      await page.goto('/new')
      await expect(page.getByRole('heading', { name: 'Create a game' })).toBeVisible()
    },
  },
  {
    name: 'Create a game, with Advanced open',
    fitsFrom: Infinity,
    actions: { 'Create game': 'main form button[type="submit"]' },
    async open(page) {
      await page.goto('/new')
      await page.getByText('Advanced', { exact: true }).click()
      await expect(page.getByRole('checkbox', { name: 'Give every seat its own link' })).toBeVisible()
    },
  },
  {
    name: 'A lobby of four seats, as its host',
    fitsFrom: Infinity,
    actions: { Ready: 'text:Ready', 'Close game': 'text:Close game' },
    async open(page) {
      await createFromForm(page, 4)
    },
  },
  {
    name: 'An invite link',
    fitsFrom: 0,
    actions: { 'Join game': 'text:Join game' },
    async open(page, context) {
      const { code } = await makeGame(context.request, unique('Host'), 'invite', 2)
      await page.goto(`/join/${code}`)
      await expect(page.getByRole('heading', { level: 1, name: 'Join a game' })).toBeVisible()
    },
  },
  {
    name: 'Sign in',
    fitsFrom: 0,
    actions: { 'Sign in': 'main form button[type="submit"]' },
    async open(page, context) {
      await signedOut(context)
      await page.goto('/login')
      await expect(page.getByRole('heading', { level: 1, name: 'Sign in' })).toBeVisible()
    },
  },
  {
    name: 'Register',
    fitsFrom: 0,
    actions: { 'Create account': 'main form button[type="submit"]' },
    async open(page, context) {
      await signedOut(context)
      await page.goto('/register')
      await expect(page.getByRole('button', { name: 'Create account' })).toBeVisible()
    },
  },
  {
    name: 'The table (dev), 4 seats, your priority',
    fitsFrom: 0,
    actions: {
      'pass button': '[data-region="controls"] [data-slot="primary-action"]',
      // The last card of the fan lies over no other card.
      hand: '[data-region="hand"] ul li:last-child button[data-object-id]',
      life: '[data-you] [data-slot="life"]',
    },
    exempt: TABLE_EXEMPT,
    async open(page) {
      await page.goto(`${DEV_WEB_URL}/dev/table?seats=4&prompt=priority&turn=you`)
      await page.locator('[data-slot="primary-action"]').waitFor()
    },
  },
  {
    name: 'A lobby (dev) of six seats',
    fitsFrom: Infinity,
    actions: { Ready: 'text:Ready', 'Close game': 'text:Close game' },
    async open(page) {
      await page.goto(`${DEV_WEB_URL}/dev/lobby?seats=6&ready=1,2`)
      await expect(page.getByRole('button', { name: 'Ready', exact: true })).toBeVisible()
    },
  },
  {
    name: 'A lobby (dev) with the longest names',
    fitsFrom: Infinity,
    actions: { Ready: 'text:Ready', 'Close game': 'text:Close game' },
    async open(page) {
      await page.goto(`${DEV_WEB_URL}/dev/lobby?seats=4&names=long&visibility=invite`)
      await expect(page.getByRole('button', { name: 'Ready', exact: true })).toBeVisible()
    },
  },
  {
    name: 'The deck builder (dev), the search pane',
    fitsFrom: 640,
    actions: { 'Search cards': 'main input[aria-label="Search cards"]' },
    afterResize: (page) => showPane(page, 'Search'),
    async open(page) {
      await page.goto(`${DEV_WEB_URL}/dev/builder`)
      await expect(page.getByRole('searchbox', { name: 'Search cards' })).toBeVisible()
    },
  },
  {
    name: 'The deck builder (dev), the deck pane',
    fitsFrom: 640,
    actions: { Save: 'text:Save' },
    afterResize: (page) => showPane(page, 'Deck'),
    async open(page) {
      await page.goto(`${DEV_WEB_URL}/dev/builder`)
      await expect(page.getByRole('button', { name: 'Save', exact: true })).toBeAttached()
    },
  },
]

/** Under the lg breakpoint the builder shows one pane and a switch picks it; from there up the switch is gone. */
async function showPane(page: Page, pane: 'Search' | 'Deck'): Promise<void> {
  const choose = page
    .getByRole('group', { name: 'Show' })
    .getByRole('button', { name: new RegExp(`^${pane}`) })
  if (await choose.isVisible()) await choose.click()
}

/** Awaits two frames, so that a layout made in reaction to a resize has been drawn before the page is read. */
const frames = (): Promise<void> =>
  new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))

/** A context whose system asks for `theme` and whose stored choice is `theme`, the way a person who set it has it. */
async function themedContext(browser: Browser, theme: Theme): Promise<BrowserContext> {
  const context = await browser.newContext({
    colorScheme: theme,
    viewport: SIZES[0] ?? { width: 1920, height: 1080 },
  })
  await context.addInitScript(storeTheme, theme)
  return context
}

/** The page shows `theme`: the class on the root element that every colour follows. */
async function expectTheme(page: Page, theme: Theme): Promise<void> {
  await expect
    .poll(() => page.evaluate(() => document.documentElement.classList.contains('dark')))
    .toBe(theme === 'dark')
}

/** Holds the page to the rules at every size, down the six and back up, and fails with each size that breaks one. */
async function holdsAtEverySize(page: Page, rules: Rules, label: string): Promise<void> {
  const broken: string[] = []
  for (const size of [...SIZES, ...SIZES.toReversed()]) {
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

for (const theme of THEMES) {
  test.describe(`in the ${theme} theme`, () => {
    for (const stop of STOPS) {
      test(`${stop.name} keeps the layout rules at every size`, async ({ browser }) => {
        const context = await themedContext(browser, theme)
        const errors: string[] = []
        try {
          const page = await context.newPage()
          page.on('pageerror', (error) => errors.push(error.message))
          await stop.open(page, context)
          await expectTheme(page, theme)
          await holdsAtEverySize(page, stop, `${stop.name} in the ${theme} theme`)
          expect(errors, 'uncaught exceptions').toEqual([])
        } finally {
          await context.close()
        }
      })
    }
  })
}

test.describe('the gate itself', () => {
  const ask = (actions: Record<string, string>, reveal = false): Ask => ({
    actions,
    reveal,
    exempt: [],
    floor: TEXT_FLOOR,
  })

  test('fails a phone page with a clipped button, a sideways scroll, small text and an action under a cover', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    await page.setContent(`<!doctype html><html><body style="margin:0"><main>
      <div style="width:300px;overflow:hidden"><button style="margin-left:250px;width:120px">Clipped</button></div>
      <div style="width:500px;height:10px"></div>
      <p style="font-size:9px">Tiny</p>
      <button id="covered">Covered</button>
      <div style="position:fixed;inset:0"></div>
    </main></body></html>`)
    const look = await page.evaluate(measure, ask({ Covered: '#covered', Absent: '#absent' }))
    expect(problemsOf(look, { fits: true })).toEqual([
      'scrolls sideways 500 > 390',
      'cut off: button "Clipped" 250..370',
      'cut off: div 0..500',
      'small text: 9px "Tiny"',
      'Covered is covered',
      'Absent is missing',
    ])
  })

  test('fails a page that must fit when it scrolls down, and reaches an action below the fold when it may scroll', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1280, height: 720 })
    await page.setContent(`<!doctype html><html><body style="margin:0"><main>
      <div style="height:1200px"></div><button id="below">Below</button>
    </main></body></html>`)
    const below = ask({ Below: '#below' })
    expect(problemsOf(await page.evaluate(measure, below), { fits: true })).toEqual([
      expect.stringMatching(/^scrolls down \d+ > 720$/),
      'Below is outside the window',
    ])
    const revealed = await page.evaluate(measure, { ...below, reveal: true })
    expect(problemsOf(revealed, { fits: false })).toEqual([])
    expect(await page.evaluate(() => window.scrollY), 'the scroll is put back').toBe(0)
  })
})

test('the game-over screen of a real game keeps the layout rules at every size in both themes', async ({
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
    const rules: Rules = {
      fitsFrom: 0,
      actions: { 'Create another game': 'main a[href="/new"]', 'Back to Play': 'main a[href="/"]' },
    }
    // The page follows the system's scheme while the choice is the system's, so the scheme is switched in place.
    for (const theme of THEMES) {
      for (const { name, page } of [ada, bo]) {
        await page.emulateMedia({ colorScheme: theme })
        await expectTheme(page, theme)
        await expect(page.locator('#over-title')).toBeVisible()
        await holdsAtEverySize(page, rules, `the game-over screen of ${name} in the ${theme} theme`)
      }
    }
    for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
