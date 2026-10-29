// Goal: accounts play real precon decks from a Postgres database through the real UI, from the lobby into the first turns (mulligans, lands, spells with their targets and payments, attacks, every step), and the run records what stops a first real game.
import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from '@playwright/test'
import type { Browser, TestInfo } from '@playwright/test'
import type { PreconList, PreconListItem } from '../src/protocol/generated'
import { playToTheEnd, Run } from './driver'
import type { Hooks, Seen, Style } from './driver'
import { everyoneReady } from './lobby'
import type { Player } from './lobby'
import { ACCOUNTS_DATABASE_URL, ACCOUNTS_WEB_URL } from './ports'

// Needs Postgres with the card data (`mtg-cli ingest` and `compile`) and the precons: set E2E_DATABASE_URL to such a
// database (docs/web.md says how). Without it `pnpm e2e` has no database and this file is skipped.
test.skip(
  ACCOUNTS_DATABASE_URL === undefined,
  'set E2E_DATABASE_URL to a database with cards and precons to run this',
)

const PASSWORD = 'correct horse battery'
/** Where the milestone screenshots go; none are taken while it is unset (the repo keeps no images). */
const SHOTS_DIR = process.env['E2E_SHOTS_DIR']
/** Decisions all seats may answer before the run is called stuck. */
const MAX_DECISIONS = 1_500
/** Time with no decision answered by any seat before the run is called stuck. */
const STALL_MS = 25_000

const unique = (prefix: string): string => `${prefix}_${Math.random().toString(36).slice(2, 8)}`

interface Account {
  name: string
  storage: Awaited<ReturnType<import('@playwright/test').BrowserContext['storageState']>>
}

/** Registers an account in a browser of its own and keeps its cookies, so a test signs it in without registering again. */
async function signUp(browser: Browser, prefix: string): Promise<Account> {
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
async function sit(browser: Browser, account: Account): Promise<Player> {
  const context = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL, storageState: account.storage })
  const page = await context.newPage()
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  return { name: account.name, page, context, errors }
}

/**
 * The precons to play, chosen by the stored data and not by name: the playable precons whose copies are mostly
 * Verified come first, the ones with the most manual copies last. The create form cannot pick a precon, so the
 * list is read from the API as the precon browser will.
 */
async function precons(browser: Browser): Promise<{ verified: PreconListItem[]; manual: PreconListItem[] }> {
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
    return { verified: byRatio, manual: byManual }
  } finally {
    await context.close()
  }
}

/**
 * Makes a Commander game as the first account through `POST /api/games` (the create form has no format and no
 * precon), has every other account open the invite page and join, and sets each seat's precon through
 * `PUT .../deck` (the lobby's deck chooser lists demo decks and the account's own). Returns the players in seat order,
 * all in the lobby.
 */
async function startTable(
  browser: Browser,
  accounts: readonly Account[],
  decks: readonly PreconListItem[],
): Promise<Player[]> {
  const [first, ...others] = accounts
  const firstDeck = decks[0]
  if (first === undefined || firstDeck === undefined) throw new Error('a table needs a seat')
  const host = await sit(browser, first)
  const created = await host.context.request.post('/api/games', {
    data: {
      format: 'commander',
      visibility: 'invite',
      held: 1,
      seats: accounts.map((account, index) => ({
        name: account.name,
        ...(index === 0 ? { deck: `precon:${firstDeck.code}` } : {}),
      })),
    },
  })
  expect(created.status(), await created.text()).toBe(201)
  const { game, code, seats } = (await created.json()) as {
    game: string
    code: string
    seats: { seat: number; token: string }[]
  }
  const hostSeat = seats[0]
  if (hostSeat === undefined) throw new Error('no seat for the host')
  await host.page.goto(`/game/${game}#seat=${hostSeat.seat}&token=${hostSeat.token}`)
  await expect(host.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()

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
    const chosen = await player.context.request.put(`/api/games/${game}/seats/${index + 1}/deck`, {
      data: { deck: `precon:${deck.code}` },
    })
    expect(chosen.status(), await chosen.text()).toBe(200)
    players.push(player)
  }
  return players
}

/** The turn number of a banner such as "Ada's turn · 5 · End step". */
const turnOf = (banner: string): number => Number(/ · (\d+) · /.exec(banner)?.[1] ?? 0)

/** Takes a milestone screenshot; the name has the number of shots so far, so the files list in the order they were taken. */
class Shots {
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

interface Played {
  run: Run
  /** Turn numbers each seat was asked a decision in. */
  turns: Map<string, Set<number>>
  /** The steps the banners named. */
  steps: Set<string>
}

/** Plays every seat until the turn number reaches `untilTurn`, with a screenshot at each first of a kind and of a turn. */
async function playTurns(
  players: readonly Player[],
  untilTurn: number,
  shots: Shots,
  styles: readonly Style[],
  run: Run,
): Promise<Played> {
  const turns = new Map<string, Set<number>>()
  const steps = new Set<string>()
  const hooks: Hooks = {
    async before(player: Player, seen: Seen, current: Run) {
      const turn = turnOf(seen.banner)
      turns.set(player.name, (turns.get(player.name) ?? new Set()).add(turn))
      steps.add(seen.banner.split(' · ').at(-1) ?? '')
      if (turn >= untilTurn) {
        current.done = true
        return
      }
      await shots.once(player, `kind:${player.name}:${seen.kind}`, seen.kind)
      await shots.once(player, `turn:${player.name}:${turn}`, `turn-${turn}-${seen.kind}`)
    },
  }
  await playToTheEnd(
    players.map((player, index) => ({ player, style: styles[index % styles.length] as Style })),
    run,
    hooks,
  )
  return { run, turns, steps }
}

/** Counts the trail lines of a seat that start with a verb ("plays Forest", "casts Sol Ring"). */
const count = (run: Run, who: string, verb: string): number =>
  run.trail.filter((line) => line.includes(` ${who} `) && new RegExp(`#\\d+ ${verb} `).test(line)).length

async function attachFailure(testInfo: TestInfo, run: Run, players: readonly Player[]): Promise<void> {
  await testInfo.attach('trail.txt', { body: run.trail.join('\n') || '(no decision was answered)' })
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

test.describe('real precon decks', () => {
  // A failed test restarts the worker and its beforeAll would register four more accounts (ten an address in 15 minutes).
  test.describe.configure({ mode: 'serial' })
  const accounts: Account[] = []
  let chosen: { verified: PreconListItem[]; manual: PreconListItem[] } = { verified: [], manual: [] }

  test.beforeAll(async ({ browser }) => {
    // A server counts registrations per address (ten in 15 minutes): the four accounts serve every test of the file.
    for (const prefix of ['ada', 'bo', 'cy', 'di']) accounts.push(await signUp(browser, prefix))
    chosen = await precons(browser)
  })

  const pairing = (label: string, pick: () => PreconListItem[]) =>
    test(`two accounts play ${label} precons through the first turns`, async ({ browser }, testInfo) => {
      test.setTimeout(4 * 60_000)
      const decks = pick().slice(0, 2)
      expect(decks, 'the database has fewer than two playable Commander precons').toHaveLength(2)
      testInfo.annotations.push({ type: 'decks', description: decks.map((deck) => deck.code).join(' vs ') })
      const players = await startTable(browser, accounts.slice(0, 2), decks)
      const shots = new Shots(SHOTS_DIR, label)
      const run = new Run(MAX_DECISIONS, STALL_MS)
      try {
        for (const player of players) await shots.once(player, `lobby:${player.name}`, 'lobby')
        await everyoneReady(players)
        for (const player of players) await shots.once(player, `opening:${player.name}`, 'opening-hands')
        const played = await playTurns(
          players,
          9,
          shots,
          [
            { blocks: true, autoPay: true, yields: false, mulligans: true },
            { blocks: false, autoPay: true, yields: false, mulligans: true },
          ],
          run,
        )
        console.log(`${label}: ${decks.map((d) => d.code).join(' vs ')}: ${played.run.summary()}`)
        console.log(`${label}: steps named by the banners: ${[...played.steps].join(', ')}`)
        // The deal is random: a hand may have no land for two turns, so the table as a whole is held to the milestones.
        const table = (verb: string): number =>
          players.reduce((sum, player) => sum + count(played.run, player.name, verb), 0)
        for (const player of players) {
          expect(
            played.turns.get(player.name)?.size ?? 0,
            `${player.name} was asked in no turn`,
          ).toBeGreaterThan(0)
        }
        expect(table('plays'), 'lands played').toBeGreaterThanOrEqual(4)
        expect(table('casts'), 'spells cast').toBeGreaterThan(0)
        // Whether a creature came down early enough to attack depends on the deal, so attacks are reported only.
        console.log(
          `${label}: lands ${table('plays')}, spells ${table('casts')}, attacks ${table('attacks with')}`,
        )
        for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
      } catch (error) {
        await attachFailure(testInfo, run, players)
        throw error
      } finally {
        if (process.env['E2E_TRAIL'] === '1') console.log(run.trail.join('\n'))
        await Promise.all(players.map(({ context }) => context.close()))
      }
    })

  pairing('verified', () => chosen.verified)
  pairing('manual', () => chosen.manual)

  test('four accounts reach the first turns of a Commander game', async ({ browser }, testInfo) => {
    test.setTimeout(5 * 60_000)
    const decks = chosen.verified.slice(0, 4)
    expect(decks, 'the database has fewer than four playable Commander precons').toHaveLength(4)
    testInfo.annotations.push({ type: 'decks', description: decks.map((deck) => deck.code).join(', ') })
    const players = await startTable(browser, accounts, decks)
    const shots = new Shots(SHOTS_DIR, 'four-seats')
    const run = new Run(MAX_DECISIONS, STALL_MS)
    try {
      for (const player of players) await shots.once(player, `lobby:${player.name}`, 'lobby')
      await everyoneReady(players)
      for (const player of players) await shots.once(player, `opening:${player.name}`, 'opening-hands')
      const played = await playTurns(
        players,
        6,
        shots,
        [{ blocks: true, autoPay: true, yields: false, mulligans: true }],
        run,
      )
      console.log(`four seats: ${decks.map((d) => d.code).join(', ')}: ${played.run.summary()}`)
      for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
    } catch (error) {
      await attachFailure(testInfo, run, players)
      throw error
    } finally {
      if (process.env['E2E_TRAIL'] === '1') console.log(run.trail.join('\n'))
      await Promise.all(players.map(({ context }) => context.close()))
    }
  })
})
