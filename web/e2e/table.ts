// Goal: the shared setup of the specs that play real decks: accounts, saved and precon decks, the create form, the lobby's chooser and the milestone screenshots.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect } from '@playwright/test'
import type { Browser, TestInfo } from '@playwright/test'
import type {
  DeckView,
  ImportResult,
  PreconList,
  PreconListItem,
  PreconView,
} from '../src/protocol/generated'
import type { Run } from './driver'
import type { Player } from './lobby'
import { ACCOUNTS_WEB_URL } from './ports'
import { ACCOUNTS_SEED } from './seeds'

export const PASSWORD = 'correct horse battery'
/** Where the milestone screenshots go; none are taken while it is unset (the repo keeps no images). */
export const SHOTS_DIR = process.env['E2E_SHOTS_DIR']

const unique = (prefix: string): string => `${prefix}_${Math.random().toString(36).slice(2, 8)}`

export interface Account {
  name: string
  storage: Awaited<ReturnType<import('@playwright/test').BrowserContext['storageState']>>
}

/** Registers an account in a browser of its own and keeps its cookies, so a test signs it in without registering again. */
export async function signUp(browser: Browser, prefix: string): Promise<Account> {
  const name = unique(prefix)
  const context = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL })
  try {
    const page = await context.newPage()
    await page.goto('/register')
    await page.getByLabel('Username').fill(name)
    await page.getByLabel('Password').fill(PASSWORD)
    await page.getByRole('button', { name: 'Create account' }).click()
    await expect(page.getByRole('heading', { level: 1, name: 'Play' })).toBeVisible()
    return { name, storage: await context.storageState() }
  } finally {
    await context.close()
  }
}

/** A player signed in as the account, in a browser context of its own, with the page's uncaught exceptions collected. */
export async function sit(browser: Browser, account: Account): Promise<Player> {
  const context = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL, storageState: account.storage })
  const page = await context.newPage()
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  const consoleErrors: string[] = []
  page.on('console', (message) => {
    // A card picture that cannot load is a network matter, not the page's.
    if (message.type() === 'error' && !/Failed to load resource/.test(message.text())) {
      consoleErrors.push(message.text())
    }
  })
  return { name: account.name, page, context, errors, consoleErrors }
}

/**
 * The precons to play, chosen by the stored data and not by name: the playable precons whose copies are mostly
 * Verified come first, the ones with the most manual copies last. The list is read from the API only to choose which
 * precons to play; the table itself is made through the create form and the lobby's chooser.
 */
export async function precons(
  browser: Browser,
): Promise<{ verified: PreconListItem[]; manual: PreconListItem[] }> {
  const context = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL })
  try {
    const response = await context.request.get('/api/precons')
    expect(response.status()).toBe(200)
    const { precons } = (await response.json()) as PreconList
    const playable = precons.filter((precon) => precon.playable && precon.commanders.length > 0)
    const ratio = (p: PreconListItem): number =>
      p.summary.counts.verified / Math.max(p.summary.counts.total, 1)
    const byRatio = [...playable].sort((a, b) => ratio(b) - ratio(a) || a.code.localeCompare(b.code))
    const byManual = [...playable].sort(
      (a, b) => b.summary.counts.manual - a.summary.counts.manual || a.code.localeCompare(b.code),
    )
    // E2E_PRECONS names the manual pair by code ("c21-silverquill-statement,sld-odds-and-ends") to replay a finding.
    const named = (process.env['E2E_PRECONS'] ?? '').split(',').filter((code) => code !== '')
    const pinned = named.flatMap((code) => playable.filter((precon) => precon.code === code))
    const replay = named.length > 0 && pinned.length === named.length
    return { verified: byRatio, manual: replay ? pinned : byManual }
  } finally {
    await context.close()
  }
}

/**
 * Chooses `deck` in the lobby's chooser as `player` does: the Precons list, a search for its name, its row (which waits
 * for the table's verdict and must not be blocked), the row's click and the preview's "Use this deck", then waits for
 * the server's answer.
 */
export async function chooseTablePrecon(player: Player, deck: PreconListItem): Promise<void> {
  const { page } = player
  await page.getByRole('tab', { name: 'Precons' }).click()
  await page.getByRole('searchbox', { name: 'Search precons' }).fill(deck.name)
  const row = page.locator(`[data-deck-option="precon:${deck.code}"]`)
  await expect(row).toBeVisible()
  await expect(row).not.toContainText('Checking at this table')
  await expect(row).not.toContainText('Cannot be taken')
  // A click selects the row for the preview beside the list; "Use this deck" there takes it once the table's verdict is in.
  await row.click()
  await expect(page.getByRole('heading', { level: 3, name: deck.name, exact: true })).toBeVisible()
  const use = page.locator('[data-slot="use-deck"]')
  await expect(use).toBeEnabled()
  await use.click()
  await expect(page.locator('[data-slot="deck-note"]')).toContainText('Your deck is now')
}

/** A deck saved in an account, which the lobby's chooser lists under "Your decks". */
export interface SavedDeck {
  id: string
  name: string
  /** The names of its commanders, for `Style.recall`. */
  commanders: string[]
}

/** What a seat plays: a precon of the server's list or a deck saved in its account. */
export type TableDeck = PreconListItem | SavedDeck

/** Whether `E2E_DECK=owner` asks for the owner's Commander deck in every seat instead of precons. */
export const OWNER_DECK = process.env['E2E_DECK'] === 'owner'

/** The owner's deck as an Arena list, the real deck whose friends' journey the owner variant holds (docs/web.md). */
const ownerList = (): string =>
  readFileSync(
    new URL('../../crates/mtg-testkit/tests/fixtures/decks/owner.arena.txt', import.meta.url),
    'utf8',
  )

/**
 * Saves the owner's deck in `account` the way the Decks page does: the importer reads the Arena list, then what it
 * resolved is stored. Every card must resolve and the list must read without a problem.
 */
export async function saveOwnerDeck(browser: Browser, account: Account): Promise<SavedDeck> {
  const context = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL, storageState: account.storage })
  const asAccount = { headers: { origin: ACCOUNTS_WEB_URL } }
  try {
    const read = await context.request.post('/api/decks/import', {
      ...asAccount,
      data: { text: ownerList(), format: 'commander' },
    })
    expect(read.ok(), `import: ${await read.text()}`).toBe(true)
    const { cards, unresolved, problems } = (await read.json()) as ImportResult
    expect(unresolved, 'every card of the owner deck is in the database').toEqual([])
    expect(problems, 'the owner deck reads without a problem').toEqual([])
    const name = `Owner deck of ${account.name}`
    const saved = await context.request.post('/api/decks', {
      ...asAccount,
      data: {
        name,
        format: 'commander',
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
    const deck = (await saved.json()) as DeckView
    const commanders = deck.cards.filter((line) => line.zone === 'commander').map((line) => line.card.name)
    expect(commanders, 'the owner deck has its commander').not.toEqual([])
    return { id: deck.id, name, commanders }
  } finally {
    await context.close()
  }
}

/**
 * Chooses a saved deck in the lobby's chooser as `player` does: the "Your decks" tab, the deck's row (which waits for
 * the table's verdict and must not be blocked), the row's click and the preview's "Use this deck", then waits for the
 * server's answer.
 */
export async function chooseSavedDeck(player: Player, deck: SavedDeck): Promise<void> {
  const { page } = player
  await page.getByRole('tab', { name: 'Your decks' }).click()
  const row = page.locator(`[data-deck-option="${deck.id}"]`)
  await expect(row).toBeVisible()
  await expect(row).not.toContainText('Checking at this table')
  await expect(row).not.toContainText('Cannot be taken')
  await row.click()
  await expect(page.getByRole('heading', { level: 3, name: deck.name, exact: true })).toBeVisible()
  const use = page.locator('[data-slot="use-deck"]')
  await expect(use).toBeEnabled()
  await use.click()
  await expect(page.locator('[data-slot="deck-note"]')).toContainText(`Your deck is now ${deck.name}`)
}

const chooseDeck = (player: Player, deck: TableDeck): Promise<void> =>
  'code' in deck ? chooseTablePrecon(player, deck) : chooseSavedDeck(player, deck)

/**
 * Makes a Commander game as the first account on the create form (Commander is its default; invite only, with the house rules filled in), has every
 * other account open the invite page with the code the host's lobby shows and join, and has every seat choose its
 * deck (a precon or a saved deck) in the lobby's deck chooser. Returns the players in seat order, all in the lobby.
 */
export async function startTable(
  browser: Browser,
  accounts: readonly Account[],
  decks: readonly TableDeck[],
  /** House rules to fill in on the create form, by the label of the rule (`{ 'Starting life': '6' }`). */
  houseRules: Readonly<Record<string, string>> = {},
): Promise<Player[]> {
  const [first, ...others] = accounts
  const firstDeck = decks[0]
  if (first === undefined || firstDeck === undefined) throw new Error('a table needs a seat')
  const host = await sit(browser, first)
  await host.page.goto('/new')
  await expect(host.page.getByRole('heading', { name: 'Create a game' })).toBeVisible()
  await expect(host.page.getByRole('radio', { name: /^Commander/ })).toBeChecked()
  await host.page.getByLabel('Seats').fill(String(accounts.length))
  await host.page.getByText('Invite only', { exact: true }).click()
  await expect(host.page.getByRole('radio', { name: 'Invite only' })).toBeChecked()
  if (Object.keys(houseRules).length > 0) {
    await host.page.locator('[data-slot="house-rules"] summary').click()
    for (const [label, value] of Object.entries(houseRules)) await host.page.getByLabel(label).fill(value)
  }
  await host.page.getByRole('button', { name: 'Create game' }).click()
  await expect(host.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
  const code = (await host.page.locator('[data-slot="invite-code"]').textContent())?.trim() ?? ''
  expect(code).not.toBe('')
  await chooseDeck(host, firstDeck)

  const players = [host]
  for (const [index, account] of others.entries()) {
    const player = await sit(browser, account)
    await player.page.goto(`/join/${code}`)
    // A signed-in account is not asked for a name: the seat is named after the account.
    await expect(player.page.getByLabel('Your name')).toHaveCount(0)
    await player.page.getByRole('button', { name: 'Join game' }).click()
    await expect(player.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    const deck = decks[index + 1]
    if (deck === undefined) throw new Error(`no precon for seat ${index + 2}`)
    await chooseDeck(player, deck)
    players.push(player)
  }
  return players
}

/** Takes a milestone screenshot; the name has the number of shots so far, so the files list in the order they were taken. */
export class Shots {
  private count = 0
  private readonly seen = new Set<string>()
  private readonly dir: string | undefined

  constructor(dir: string | undefined, table: string) {
    this.dir = dir === undefined ? undefined : join(dir, table)
    if (this.dir !== undefined) mkdirSync(this.dir, { recursive: true })
  }

  /** Shoots `player`'s page once for each `key`. */
  async once(player: Player, key: string, label: string): Promise<void> {
    if (this.dir === undefined || this.seen.has(key)) return
    this.seen.add(key)
    this.count += 1
    const file = `${String(this.count).padStart(2, '0')}-${player.name.split('_')[0]}-${label}.png`
    await player.page.screenshot({ path: join(this.dir, file) })
  }
}

export async function attachFailure(testInfo: TestInfo, run: Run, players: readonly Player[]): Promise<void> {
  await testInfo.attach('trail.txt', { body: run.trail.join('\n') || '(no decision was answered)' })
  // The server's seed makes the game replayable: `E2E_SEED=<it>` runs the same shuffles again.
  await testInfo.attach('seed.txt', { body: `MTG_SEED=${ACCOUNTS_SEED || 'random'}` })
  for (const { name, page } of players) {
    const png = await page.screenshot()
    await testInfo.attach(`${name}.png`, { body: png, contentType: 'image/png' })
    // The page's words say what a screenshot may hide (a prompt cut off, a seat waiting on a decision).
    await testInfo.attach(`${name}.txt`, { body: await page.locator('body').innerText() })
    if (SHOTS_DIR !== undefined) {
      mkdirSync(SHOTS_DIR, { recursive: true })
      writeFileSync(join(SHOTS_DIR, `failure-${name}.png`), png)
      writeFileSync(join(SHOTS_DIR, `failure-${name}.txt`), await page.locator('body').innerText())
    }
  }
}

/**
 * The `count` playable Commander precons with the highest share of Verified copies whose only commander costs at most
 * `maxValue` and whose deck check has no issue, so that a commander is cast early and again with its tax. The list
 * says nothing of a commander's cost, so each candidate's detail is read.
 */
export async function smallCommanderPrecons(
  browser: Browser,
  count: number,
  maxValue: number,
): Promise<PreconListItem[]> {
  const { verified } = await precons(browser)
  const context = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL })
  try {
    const small: PreconListItem[] = []
    for (const candidate of verified.slice(0, 60)) {
      if (small.length === count) break
      const response = await context.request.get(`/api/precons/${candidate.code}`)
      if (response.status() !== 200) continue
      const view = (await response.json()) as PreconView
      const commanders = view.boards.filter((board) => board.board === 'commander').flatMap((b) => b.cards)
      const [only] = commanders
      const cost = only?.card?.mana_value ?? Infinity
      if (view.issues.length === 0 && commanders.length === 1 && cost <= maxValue) small.push(candidate)
    }
    return small
  } finally {
    await context.close()
  }
}
