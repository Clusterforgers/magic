// Goal: the reach gate. At every screen of `SCREENS` (short and small laptops, a 1920x1080 desktop zoomed to 125 to 200 %, windows under browser bars, high-DPI, 16:10, 3:2, ultrawide, a tablet and a phone each way up) every control a page draws can be seen and pressed, and no line of text is cut across: nothing is outside the window or cut off by a box that does not scroll, nothing is covered by another element, the page never scrolls sideways, and a page meant to fit never scrolls down. Held for the table in every mode (grid, duel, focus, teams, Two-Headed Giant, a boss fight) at every decision that has a home of its own, with its menus, popovers and dialogs open, for the game-over screen, and for the pages (Play, Create, Learn, sign-in, a lobby, the builder, precons, the review screens, Accounts, a delve in every state). The table at the decisions that crowd its dock most (priority, the combat declarations, targets, a payment, the opening hand, a boss fight) is held to every screen, the other stops to the `core` ones.
import { expect, test } from '@playwright/test'
import type { CDPSession, Page } from '@playwright/test'
import { reach, SCREENS } from './reachAudit'
import type { Reach, ReachAsk, Screen } from './reachAudit'
import { DEV_WEB_URL } from './ports'

/** A page of the gate and the way to it. */
interface Stop {
  name: string
  /** The page is as high as the window and never scrolls down (the table); else it scrolls as a document. */
  fits: boolean
  /** Where it is: a path of the preview, or a URL of the dev server. */
  url: string
  /** What is drawn once the page is up. */
  ready: string
  /** Something to open on it (a menu, a popover, a dialog): `press` is clicked while `shows` is not drawn. */
  open?: { press: string; shows: string }
  /** Held to every screen of `SCREENS`; the other stops to the `core` ones. */
  every?: true
  /**
   * Not held on a phone: there the decision is a bar of one height under the hand, which `decision-home.spec.ts`,
   * `phone-arrows.spec.ts` and `layout.spec.ts` hold at 390x844.
   */
  desk?: true
}

/**
 * The cards of the table are not controls of its frame: a board fits them by its own rules (lanes, stacks, a zone
 * that scrolls; `fit.spec.ts`, `crowded-fit.spec.ts`), the hand is a fan of cards that overlap, and a card's face
 * clips its own art and text.
 */
const TABLE_CARDS = [
  '[data-battlefield-seat] [data-object-id]',
  '[data-region="hand"]',
  '[data-slot="card-face"]',
  '[data-slot="tile-strip"]',
]

/** Layers that lie over the page while they are up; what is under one is not judged covered. */
const OVERLAYS = [
  '[role="dialog"]',
  '[role="menu"]',
  '[role="listbox"]',
  '[role="tooltip"]',
  '[data-slot="popover-content"]',
  // The backdrop of a menu or a dialog: the page under it waits for the layer to close.
  '[data-base-ui-inert]',
  '[data-slot="dialog-overlay"]',
  '[data-slot="choice-host"]',
  '[data-slot="choice-overlay"]',
  // The side area where it has no column: a panel from the rail and a sheet from the top bar, a phone's payment sheet.
  '[data-slot="rail-panel"]',
  '[data-slot="side-overlay"]',
  '[data-slot="decision-sheet"]',
]

const CONTROLS = '[data-region="controls"]'

const table = (name: string, query: string, open?: Stop['open']): Stop => ({
  name: `the table, ${name}`,
  fits: true,
  url: `${DEV_WEB_URL}/dev/table?${query}`,
  ready: CONTROLS,
  ...(open === undefined ? {} : { open }),
})

/** A table stop held to every screen. */
const crowded = (name: string, query: string): Stop => ({ ...table(name, query), every: true })

/** A stop held off a phone alone. */
const desk = (stop: Stop): Stop => ({ ...stop, desk: true })

const dev = (name: string, path: string, ready = 'main h1'): Stop => ({
  name,
  fits: false,
  url: `${DEV_WEB_URL}${path}`,
  ready,
})

const page = (name: string, path: string, ready = 'main h1'): Stop => ({
  name,
  fits: false,
  url: path,
  ready,
})

const PRIORITY = 'seats=4&prompt=priority&turn=you'

const STOPS: readonly Stop[] = [
  crowded('4 seats, your priority', PRIORITY),
  desk(table('4 seats, a take-back offered', `${PRIORITY}&takeback=1`)),
  table('4 seats, the table menu open', PRIORITY, {
    press: 'button[aria-label="Table menu"]',
    shows: '[data-slot="popover-content"][data-open]',
  }),
  table('4 seats, the Stops popover open', PRIORITY, {
    press: `${CONTROLS} [data-slot="stops-button"]`,
    shows: '[data-slot="popover-content"][data-open] [data-slot="stop-ladder"]',
  }),
  table('4 seats, the other ways to pass open', PRIORITY, {
    press: `${CONTROLS} [data-slot="more-passing"]`,
    shows: '[role="menu"]',
  }),
  table('4 seats, the help menu open', PRIORITY, {
    press: '[data-slot="help-menu"]',
    shows: '[role="menu"]',
  }),
  table('4 seats, your graveyard open', PRIORITY, {
    press: '[data-you] button[aria-label^="Graveyard"]',
    shows: '[data-slot="dialog-content"][data-open]',
  }),
  crowded('the duel board, your priority', 'board=duel&prompt=priority&turn=you'),
  desk(table('the duel board, declaring attackers', 'board=duel&prompt=attack&turn=you')),
  crowded('2 seats, the opening hand', 'seats=2&prompt=mulligan'),
  desk(crowded('4 seats, declaring attackers', 'seats=4&prompt=attack&turn=you')),
  crowded('4 seats, declaring blockers', 'seats=4&prompt=block'),
  table('4 seats, declaring blockers with the side column folded', 'seats=4&prompt=block&side=folded'),
  table('the heavy board, declaring blockers', 'board=heavy&prompt=block'),
  desk(crowded('4 seats, choosing targets', 'seats=4&prompt=targets')),
  table('4 seats, choosing targets with the side column folded', 'seats=4&prompt=targets&side=folded'),
  table('4 seats, assigning damage', 'seats=4&prompt=damage'),
  table('4 seats, a discard', 'seats=4&prompt=discard'),
  table('4 seats, bottoming cards', 'seats=4&prompt=bottom'),
  table('4 seats, a yes-or-no question', 'seats=4&prompt=ask-may'),
  crowded('6 seats, paying a cost', 'seats=6&prompt=pay'),
  table('4 seats, paying a cost with the side column folded', 'seats=4&prompt=pay&side=folded'),
  table('12 seats in focus mode', 'seats=12&prompt=priority&turn=you'),
  table('two teams of two', 'teams=2v2&prompt=priority&turn=you'),
  table('Two-Headed Giant', 'teams=2hg&prompt=priority&turn=you'),
  crowded('a boss fight', 'boss=3&prompt=priority&turn=you'),
  table('the Commands panel after a look at the top', 'panel=top'),
  table('the Commands panel with a card picked', 'panel=card'),
  {
    name: 'the game-over screen',
    fits: true,
    url: `${DEV_WEB_URL}/dev/table?result=won`,
    ready: '#over-title',
  },
  dev('a lobby of six seats', '/dev/lobby?seats=6&ready=1,2'),
  dev(
    'a Commander lobby with a long list of decks',
    '/dev/lobby?seats=4&me=0&nodeck=0&account=1&format=commander&decks=many',
  ),
  dev('the deck builder', '/dev/builder', 'main'),
  dev('the precons', '/dev/precons'),
  dev('the cards to check', '/dev/unvalidated?view=reads'),
  dev('the review screen', '/dev/read', 'main'),
  dev('the review dialog from the table', '/dev/read?view=dialog', '[role="dialog"]'),
  dev('the Accounts page with thirty friends', '/dev/accounts?many=1&names=long'),
  ...['lobby', 'map', 'relics', 'shop', 'campfire', 'event'].map((state) =>
    dev(`a delve in the ${state} state`, `/dev/delve?state=${state}`, 'main button'),
  ),
  page('Play', '/'),
  page('Create a game', '/new'),
  page('Learn', '/learn'),
  page('Sign in', '/login'),
  page('Register', '/register'),
]

/** Awaits two frames, so that a layout made in reaction to a resize has been drawn before the page is read. */
const frames = (): Promise<void> =>
  new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))

/** Gives the page the screen: its size in CSS px and its device pixels per px, as a zoomed or high-DPI window has. */
async function show(session: CDPSession, screen: Screen): Promise<void> {
  await session.send('Emulation.setDeviceMetricsOverride', {
    width: screen.width,
    height: screen.height,
    deviceScaleFactor: screen.scale ?? 1,
    mobile: false,
  })
}

/** Opens what the stop looks at when a resize closed it or it was never open. */
async function opened(page: Page, stop: Stop): Promise<void> {
  if (stop.open === undefined) return
  if (await page.locator(stop.open.shows).first().isVisible()) return
  await page.locator(stop.open.press).first().click()
  await page.locator(stop.open.shows).first().waitFor()
}

/**
 * Measures until the page keeps the rules or the time is up, and returns what is still wrong: a page lays itself out
 * a frame or more after the window changes, and a transition can be under way.
 */
async function settled(page: Page, ask: ReachAsk, timeoutMs = 4_000): Promise<Reach> {
  const deadline = Date.now() + timeoutMs
  for (;;) {
    await page.evaluate(frames)
    const found = await page.evaluate(reach, ask)
    if (found.findings.length === 0 || Date.now() >= deadline) return found
    await page.waitForTimeout(100)
  }
}

// A trace of every resize of every stop costs more than the gate itself; a failure names the screen and the control.
test.use({ trace: 'off' })

for (const stop of STOPS) {
  test(`${stop.name}: every control is in reach at every screen`, async ({ page }) => {
    test.setTimeout(240_000)
    const errors: string[] = []
    page.on('pageerror', (error) => errors.push(error.message))
    const session = await page.context().newCDPSession(page)
    await page.goto(stop.url)
    await page.locator(stop.ready).first().waitFor()
    // The dev route's own tools are for looking at the table, not part of it.
    await page.addStyleTag({ content: '[data-dev-tool] { display: none !important; }' })
    const ask: ReachAsk = {
      exempt: TABLE_CARDS,
      overlays: OVERLAYS,
      scrolls: !stop.fits,
    }
    const broken: string[] = []
    let judged = 0
    const held = SCREENS.filter(
      (one) => (stop.every === true || one.core === true) && !(stop.desk === true && one.phone === true),
    )
    for (const screen of held) {
      await show(session, screen)
      await opened(page, stop)
      const found = await settled(page, ask)
      judged += found.controls
      if (found.findings.length > 0) broken.push(`${screen.name}: ${found.findings.join('; ')}`)
    }
    expect(judged, 'controls judged').toBeGreaterThan(0)
    expect(broken, stop.name).toEqual([])
    expect(errors, 'uncaught exceptions').toEqual([])
  })
}

test.describe('the gate itself', () => {
  const ask = (scrolls: boolean): ReachAsk => ({
    exempt: ['[data-exempt]'],
    overlays: ['[data-layer]'],
    scrolls,
  })

  test('names a control outside the window, one cut off, one covered, one too big for its scrolling box, a cut line and a sideways scroll', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 800, height: 400 })
    await page.setContent(`<!doctype html><html><body style="margin:0;font:16px/20px sans-serif"><main>
      <div style="height:40px;overflow:hidden"><p style="margin:30px 0 0">Half a line</p></div>
      <div style="height:40px;overflow:hidden"><button style="margin-top:30px;height:30px">Clipped</button></div>
      <div style="height:20px;overflow:auto"><button style="height:60px">Tall</button></div>
      <div style="height:30px;overflow:auto"><div style="height:90px"></div><button style="height:24px">Scrolled to</button></div>
      <p style="width:60px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">Ends in an ellipsis on purpose</p>
      <div data-exempt style="height:10px;overflow:hidden"><button style="height:30px">Exempt</button></div>
      <button id="under" style="position:absolute;left:300px;top:0;width:100px;height:30px">Under</button>
      <div style="position:absolute;left:290px;top:0;width:120px;height:40px;background:#000"></div>
      <button style="position:absolute;left:500px;top:0;width:100px;height:30px">Under a layer</button>
      <div data-layer style="position:absolute;left:490px;top:0;width:120px;height:40px"></div>
      <details><summary>More</summary><button>Folded away</button></details>
      <button style="position:absolute;left:20px;top:500px">Below the window</button>
      <div style="width:900px;height:1px"></div>
    </main></body></html>`)
    const found = await page.evaluate(reach, ask(false))
    const kinds = found.findings.map((finding) => finding.replace(/ at -?\d+,-?\d+( \d+x\d+)?/, ''))
    expect(kinds).toEqual([
      'cut off: button "Clipped", 33 % in view, by div "Clipped"',
      'cut off: button "Tall", 33 % in view, by div "Tall"',
      'covered: button "Under", 0 of 9 points free, under div',
      'cut off: button "Below the window", 0 % in view, by the window',
      'cut text: "Half a line", by div "Half a line"',
      'scrolls sideways 900 > 800',
      expect.stringMatching(/^scrolls down \d+ > 400$/),
    ])
  })

  test('reaches a control below the fold of a page that scrolls, past a footer that stays', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 800, height: 400 })
    await page.setContent(`<!doctype html><html><body style="margin:0"><main>
      <div style="height:380px"></div><button style="height:30px">Under the footer until scrolled</button>
      <div style="height:600px"></div><button>Below</button>
      <div style="position:sticky;bottom:0;height:60px;background:#000"><button>Save</button></div>
    </main></body></html>`)
    expect((await page.evaluate(reach, ask(true))).findings).toEqual([])
    expect(await page.evaluate(() => window.scrollY), 'the scroll is put back').toBe(0)
  })
})
