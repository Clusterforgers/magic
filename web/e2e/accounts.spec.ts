// Goal: accounts work end to end in a browser on a real server and database: register, create a game, come back through My games and rejoin with no seat link, two accounts at one table, and signing out and in again; and the pages that need an account (Decks, My games, the builder with a real imported deck) keep the layout rules of the gate at the six sizes in both themes.
import { readFileSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import type { Browser, BrowserContext, Page } from '@playwright/test'
import type { DeckView, ImportResult } from '../src/protocol/generated'
import { THEMES } from './layoutAudit'
import { expectTheme, holdsAtEverySize, showPane, themedContext } from './layoutGate'
import type { Rules } from './layoutGate'
import { ACCOUNTS_DATABASE_URL, ACCOUNTS_WEB_URL } from './ports'

// Accounts need Postgres, so this spec runs only against its own server on an empty scratch database: set
// E2E_DATABASE_URL (docs/web.md says how). Without it `pnpm e2e` has no database and this file is skipped.
test.skip(
  ACCOUNTS_DATABASE_URL === undefined,
  'set E2E_DATABASE_URL to an empty scratch database to run this',
)

const PASSWORD = 'correct horse battery'

/** A username no earlier run used, so a scratch database that is kept between runs does not say "taken". */
const unique = (prefix: string): string => `${prefix}_${Math.random().toString(36).slice(2, 8)}`

/** One browser, with its own cookies, and the page's uncaught exceptions collected: the UI should never throw. */
interface Visitor {
  context: BrowserContext
  page: Page
  errors: string[]
}

async function visitor(browser: Browser): Promise<Visitor> {
  const context = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL })
  const page = await context.newPage()
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  return { context, page, errors }
}

const account = (page: Page) => page.getByRole('navigation', { name: 'Account' })

async function register(page: Page, username: string): Promise<void> {
  await page.goto('/register')
  await page.getByLabel('Username').fill(username)
  await page.getByLabel('Password').fill(PASSWORD)
  await page.getByRole('button', { name: 'Create account' }).click()
  // Registering signs in and goes home, which is the create page.
  await expect(page.getByRole('form', { name: 'Create a game' })).toBeVisible()
  await expect(account(page).getByText(username)).toBeVisible()
}

/** Creates a two-seat game on `/` as the signed-in user and returns the seat links, `/game/<id>#seat=<n>&token=<t>`. */
async function createGame(page: Page): Promise<string[]> {
  // A link for every seat, so the page lists the links this helper returns.
  await page.getByText('Advanced', { exact: true }).click()
  await page.getByRole('checkbox', { name: 'Give every seat its own link' }).check()
  await page.getByRole('button', { name: 'Create game' }).click()
  await expect(page.getByRole('heading', { name: 'Game created' })).toBeVisible()
  const links = page.getByRole('list', { name: 'Seat links' }).locator('a[data-share-link]')
  await expect(links).toHaveCount(2)
  return links.evaluateAll((anchors) => anchors.map((anchor) => anchor.getAttribute('href') ?? ''))
}

const games = (page: Page) => page.locator('main [data-game]')
const gameIdOf = (page: Page): string => new URL(page.url()).pathname.split('/').at(-1) ?? ''

test('an account creates a game, closes the page, finds it under My games and rejoins it without a link', async ({
  browser,
}) => {
  const ada = unique('ada')
  const { context, page, errors } = await visitor(browser)
  try {
    await register(page, ada)
    await expect(page.locator('[data-slot="signed-in-note"]')).toContainText(
      `You will play as ${ada} in seat 1`,
    )

    await createGame(page)
    await expect(page.locator('[data-slot="saved-note"]')).toContainText('saved to your account')
    await page.getByRole('button', { name: `Open my seat (${ada})` }).click()
    await expect(page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    await expect(page.locator('[data-slot="seat-saved"]')).toContainText('This seat is saved to your account')
    const game = gameIdOf(page)
    await page.close()

    // A new page of the same browser: the cookie is all that is left of the first.
    const back = await context.newPage()
    back.on('pageerror', (error) => errors.push(error.message))
    await back.goto('/games')
    await expect(games(back)).toHaveCount(1)
    const row = games(back).first()
    await expect(row).toHaveAttribute('data-game', game)
    await expect(row).toHaveAttribute('data-status', 'lobby')
    await expect(row).toContainText('Lobby')
    await expect(row).toContainText('Player 2')
    await expect(row).toContainText(`${ada}, seat 1`)

    await row.getByRole('link', { name: /continue/i }).click()
    await expect(back.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    // No seat link: the address is the game and nothing else, and the page is connected all the same.
    expect(new URL(back.url()).pathname).toBe(`/game/${game}`)
    expect(new URL(back.url()).hash).toBe('')
    await expect(back.getByRole('list', { name: 'Seats' }).getByRole('listitem').first()).toContainText('you')
    await expect(back.locator('[data-slot="lobby-summary"]')).toHaveText(
      '0 of 2 ready, waiting for you to press Ready and Player 2 to join',
    )
    expect(errors).toEqual([])
  } finally {
    await context.close()
  }
})

test('a second account joins by seat link, and both accounts list the game, in the lobby and once it runs', async ({
  browser,
}) => {
  const adaName = unique('ada')
  const boName = unique('bo')
  const ada = await visitor(browser)
  const bo = await visitor(browser)
  try {
    await register(ada.page, adaName)
    const links = await createGame(ada.page)
    await ada.page.getByRole('button', { name: `Open my seat (${adaName})` }).click()
    await expect(ada.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    const game = gameIdOf(ada.page)

    // Bo opens seat 2's link while signed in: the server saves that seat to bo's account.
    await register(bo.page, boName)
    const link = links[1]
    if (link === undefined || !link.startsWith('/game/')) throw new Error(`no link for seat 2: ${link}`)
    await bo.page.goto(link)
    await expect(bo.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    await expect(bo.page.locator('[data-slot="seat-saved"]')).toContainText(
      'This seat is saved to your account',
    )
    // Ada's lobby learns who sits where when bo connects (it polls every 3 s).
    const seats = ada.page.getByRole('list', { name: 'Seats' }).getByRole('listitem')
    await expect(seats.nth(1)).toContainText(`signed in as ${boName}`, { timeout: 15_000 })
    await expect(seats.nth(0)).toContainText(`signed in as ${adaName}`)

    // Both lists have the game, each naming the other account.
    await ada.page.goto('/games')
    await bo.page.goto('/games')
    await expect(games(ada.page)).toHaveCount(1)
    await expect(games(bo.page)).toHaveCount(1)
    await expect(games(ada.page).first()).toHaveAttribute('data-game', game)
    await expect(games(bo.page).first()).toHaveAttribute('data-game', game)
    await expect(games(ada.page).first()).toContainText(boName)
    await expect(games(bo.page).first()).toContainText(adaName)
    await expect(games(bo.page).first()).toContainText('Player 2, seat 2')

    // Start it from the two lobbies, which both lists then show as running.
    await ada.page.getByRole('link', { name: /continue/i }).click()
    await bo.page.getByRole('link', { name: /continue/i }).click()
    for (const { page } of [ada, bo]) {
      await page.getByRole('button', { name: 'Ready', exact: true }).click()
    }
    for (const { page } of [ada, bo]) {
      await expect(page.getByRole('heading', { name: 'Opening hands' })).toBeVisible()
    }
    for (const { page } of [ada, bo]) {
      await page.goto('/games')
      await expect(games(page).first()).toHaveAttribute('data-status', 'running')
      await expect(games(page).first()).toContainText('Running')
    }

    // Back into the running game by the session alone, on both sides.
    for (const { page } of [ada, bo]) {
      await games(page)
        .first()
        .getByRole('link', { name: /continue/i })
        .click()
      await expect(page.getByRole('heading', { name: 'Opening hands' })).toBeVisible()
      expect(new URL(page.url()).hash).toBe('')
    }
    expect([...ada.errors, ...bo.errors]).toEqual([])
  } finally {
    await Promise.all([ada.context.close(), bo.context.close()])
  }
})

test('signing out leaves a guest, My games asks for a sign-in and comes back to the list after it', async ({
  browser,
}) => {
  const adaName = unique('ada')
  const { context, page, errors } = await visitor(browser)
  try {
    await register(page, adaName)
    await createGame(page)
    await page.goto('/games')
    await expect(games(page)).toHaveCount(1)

    await account(page)
      .getByRole('button', { name: /signed in as/i })
      .click()
    await page.getByRole('menuitem', { name: 'Sign out' }).click()
    await expect(page.getByRole('form', { name: 'Create a game' })).toBeVisible() // signing out on /games goes home
    await expect(account(page).getByRole('link', { name: 'Sign in' })).toBeVisible()
    await expect(account(page).getByText(adaName)).toHaveCount(0)

    await page.goto('/games')
    await expect(page.getByRole('form', { name: 'Sign in' })).toBeVisible()
    expect(new URL(page.url()).pathname).toBe('/login')
    expect(new URL(page.url()).searchParams.get('next')).toBe('/games')

    // A wrong password says so in one sentence and keeps the username; the right one goes back to the list.
    await page.getByLabel('Username').fill(adaName)
    await page.getByLabel('Password').fill('not the right one!')
    await page.getByRole('button', { name: 'Sign in' }).click()
    await expect(page.getByRole('alert')).toContainText('The username or password is wrong')
    await expect(page.getByLabel('Password')).toHaveValue('')
    await page.getByLabel('Password').fill(PASSWORD)
    await page.getByRole('button', { name: 'Sign in' }).click()
    await expect(games(page)).toHaveCount(1)
    expect(new URL(page.url()).pathname).toBe('/games')
    expect(errors).toEqual([])
  } finally {
    await context.close()
  }
})

// The layout gate for the pages that need an account, as `layout.spec.ts` holds the pages that need none. The account
// has a long name and the lists hold the longest names, since those break a layout first.

const COMMANDER_LIST = readFileSync(new URL('./fixtures/commander-deck.txt', import.meta.url), 'utf8')

const MODERN_LIST = [
  '4 Lightning Bolt',
  '4 Monastery Swiftspear',
  '4 Goblin Guide',
  '4 Lava Spike',
  '4 Chain Lightning',
  '4 Rift Bolt',
  '4 Boros Charm',
  '4 Skullcrack',
  '4 Searing Blaze',
  '20 Mountain',
].join('\n')

/** The longest name a deck can have, in one word. */
const LONGEST_DECK_NAME = `Supercalifragilistic${'x'.repeat(80)}`
const TOODEEP = 'Krark Sakashima Storm [TOODEEP]'

const asAccount = { headers: { origin: ACCOUNTS_WEB_URL } }

/** Registers `username` in the context, which then holds the session cookie of its requests and of its pages. */
async function signUp(context: BrowserContext, username: string): Promise<void> {
  const made = await context.request.post('/api/auth/register', {
    ...asAccount,
    data: { username, password: PASSWORD },
  })
  expect(made.ok(), 'register').toBe(true)
}

/** Saves a deck the way a person does: the importer reads the list and what it resolved is stored. */
async function saveDeck(
  context: BrowserContext,
  deck: { name: string; format: string; list: string },
): Promise<string> {
  const read = await context.request.post('/api/decks/import', {
    ...asAccount,
    data: { text: deck.list, format: deck.format },
  })
  expect(read.ok(), 'import').toBe(true)
  const { cards, unresolved } = (await read.json()) as ImportResult
  expect(unresolved, 'every card of the list is in the database').toEqual([])
  const saved = await context.request.post('/api/decks', {
    ...asAccount,
    data: {
      name: deck.name,
      format: deck.format,
      notes: '',
      cards: cards.map((line) => ({
        zone: line.zone,
        oracle_id: line.card.oracle_id,
        printing_id: line.printing_id,
        count: line.count,
      })),
    },
  })
  expect(saved.ok(), `save: ${await saved.text()}`).toBe(true)
  return ((await saved.json()) as DeckView).id
}

/** A public game of `seats` seats made by the signed-in account, so seat 1 is its own and the others are open. */
async function makeGame(context: BrowserContext, seats: number, name: string): Promise<void> {
  const made = await context.request.post('/api/games', {
    ...asAccount,
    data: {
      format: 'normal',
      visibility: 'public',
      held: 1,
      seats: Array.from({ length: seats }, (_, index) => ({
        name: index === 0 ? name : `Bartholomew-the-Long-Name-${index}`,
        deck: index % 2 === 0 ? 'mono-green' : 'mono-red',
      })),
    },
  })
  expect(made.ok(), 'create game').toBe(true)
}

type StorageState = Awaited<ReturnType<BrowserContext['storageState']>>

/** The two accounts of the gate: one with nothing in it, one with decks and games, as the cookies of their sessions. */
interface Accounts {
  empty: StorageState
  full: StorageState
}

/** Registers both accounts once for the whole gate, since a server counts registrations per address (ten in 15 minutes). */
async function makeAccounts(browser: Browser): Promise<Accounts> {
  const fresh = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL })
  const used = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL })
  try {
    await signUp(fresh, unique('layout_empty'))
    const name = unique('long_layout_account_name')
    await signUp(used, name)
    // The commander deck last, so it is the first of the list: the one a click on the list opens.
    for (const index of [1, 2, 3, 4, 5, 6])
      await saveDeck(used, { name: `Burn ${index}`, format: 'modern', list: MODERN_LIST })
    await saveDeck(used, { name: TOODEEP, format: 'commander', list: COMMANDER_LIST })
    await saveDeck(used, { name: LONGEST_DECK_NAME, format: 'commander', list: COMMANDER_LIST })
    for (const seats of [2, 3, 4, 6, 2, 5, 12, 2]) await makeGame(used, seats, name)
    return { empty: await fresh.storageState(), full: await used.storageState() }
  } finally {
    await Promise.all([fresh.close(), used.close()])
  }
}

/** A page of the gate and the way to it. */
interface AccountStop extends Rules {
  name: string
  /** Whose session opens the page. */
  account: keyof Accounts
  /** Opens the page and waits until it is drawn. */
  open(page: Page): Promise<void>
}

const DECK_ACTIONS = { 'New deck': 'text:New deck', 'Import deck': 'text:Import deck' }

/** The builder of the first deck of the list. */
async function openBuilder(page: Page): Promise<void> {
  await page.goto('/decks')
  await page.getByRole('list', { name: 'Your decks' }).getByRole('link').first().click()
  await expect(page.getByRole('button', { name: 'Save', exact: true })).toBeAttached()
}

const ACCOUNT_STOPS: readonly AccountStop[] = [
  {
    name: 'Decks, with none',
    account: 'empty',
    fitsFrom: 640,
    actions: DECK_ACTIONS,
    async open(page) {
      await page.goto('/decks')
      await expect(page.getByText('You have no decks yet')).toBeVisible()
    },
  },
  {
    name: 'Decks, with a list of them',
    account: 'full',
    fitsFrom: 640,
    actions: { ...DECK_ACTIONS, 'Edit a deck': 'main ul[aria-label="Your decks"] a[aria-label^="Edit "]' },
    async open(page) {
      await page.goto('/decks')
      await expect(page.getByRole('list', { name: 'Your decks' }).getByRole('listitem')).toHaveCount(8)
    },
  },
  {
    name: 'My games, with none',
    account: 'empty',
    fitsFrom: 0,
    actions: { 'Create game': 'main a[href="/new"]' },
    async open(page) {
      await page.goto('/games')
      await expect(page.getByText('You have no games yet')).toBeVisible()
    },
  },
  {
    name: 'My games, with a list of them',
    account: 'full',
    fitsFrom: 0,
    actions: { 'Create game': 'main a[href="/new"]', Continue: 'main [data-game] a' },
    async open(page) {
      await page.goto('/games')
      await expect(games(page)).toHaveCount(8)
    },
  },
  {
    name: 'The builder with a real deck, the search pane',
    account: 'full',
    fitsFrom: 640,
    actions: { 'Search cards': 'main input[aria-label="Search cards"]' },
    afterResize: (page) => showPane(page, 'Search'),
    async open(page) {
      await openBuilder(page)
      await showPane(page, 'Search')
      await page.getByRole('searchbox', { name: 'Search cards' }).fill('dragon')
      await expect(
        page.getByRole('list', { name: 'Search results' }).getByRole('listitem').first(),
      ).toBeVisible()
    },
  },
  {
    name: 'The builder with a real deck, the deck pane',
    account: 'full',
    fitsFrom: 640,
    actions: { Save: 'text:Save' },
    afterResize: (page) => showPane(page, 'Deck'),
    async open(page) {
      await openBuilder(page)
      await showPane(page, 'Deck')
      await expect(
        page.getByRole('list', { name: 'Main deck cards' }).getByRole('listitem').first(),
      ).toBeVisible()
    },
  },
]

test.describe('the layout gate', () => {
  let accounts: Accounts
  test.beforeAll(async ({ browser }) => {
    accounts = await makeAccounts(browser)
  })

  for (const theme of THEMES) {
    for (const stop of ACCOUNT_STOPS) {
      test(`${stop.name} keeps the layout rules at every size in the ${theme} theme`, async ({ browser }) => {
        test.setTimeout(120_000)
        const context = await themedContext(browser, theme, {
          baseURL: ACCOUNTS_WEB_URL,
          storageState: accounts[stop.account],
        })
        const errors: string[] = []
        try {
          const page = await context.newPage()
          page.on('pageerror', (error) => errors.push(error.message))
          await stop.open(page)
          await expectTheme(page, theme)
          await holdsAtEverySize(page, stop, `${stop.name} in the ${theme} theme`)
          expect(errors, 'uncaught exceptions').toEqual([])
        } finally {
          await context.close()
        }
      })
    }
  }
})
