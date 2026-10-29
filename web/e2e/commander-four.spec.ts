// Goal: four accounts play a four-seat Commander game with real precon decks in the real client against a real server, from the create form and the lobby through mulligans, casts from the command zone with their tax and commander damage to a winner, with every seat seeing the same game, no page scrolling and no console error.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import type { PreconListItem } from '../src/protocol/generated'
import { playToTheEnd, Run, turnOf } from './driver'
import type { Hooks, Seen, Style } from './driver'
import { measure, settle, TABLE_EXEMPT, TEXT_FLOOR } from './layoutAudit'
import type { Look } from './layoutAudit'
import { everyoneReady } from './lobby'
import type { Player } from './lobby'
import { ACCOUNTS_DATABASE_URL } from './ports'
import { attachFailure, signUp, smallCommanderPrecons, startTable } from './table'
import type { Account } from './table'

// Needs Postgres with the card data (`mtg-cli ingest` and `compile`) and the precons: set E2E_DATABASE_URL to such a
// database (docs/web.md says how). Without it `pnpm e2e` has no database and this file is skipped.
test.skip(
  ACCOUNTS_DATABASE_URL === undefined,
  'set E2E_DATABASE_URL to a database with cards and precons to run this',
)

/** House rules that shorten the game: a low life total, so the attacks of a few turns end it. */
const HOUSE_RULES = { 'Starting life': process.env['E2E_LIFE'] ?? '12' }
/** The turn the run stops at without a winner, which is a failure that names the table. */
const TURN_CAP = Number(process.env['E2E_TURN_CAP'] ?? 60)
/** Decisions all seats may answer before the run is called stuck. */
const MAX_DECISIONS = 4_000
/** Time with no decision answered by any seat before the run is called stuck. */
const STALL_MS = 40_000
/** The commander of seats 2 and 3 goes back to the command zone once, by hand, so that its second cast shows the tax. */
const RECALLING = [2, 3]

/** What the table shows of the game, normalised so that every seat's page reads the same: the turn and each life. */
interface Table {
  /** "Bo's turn · 5 · End step", with "Your turn" written out as the viewer's name. */
  turn: string
  /** Each pod's seat and life. */
  lives: string[]
  /** Entries in the game log. */
  log: number
  over: boolean
}

/** Runs in the page: the turn badge, the life of each seat's pod or tile, and the number of log lines. */
function tableOf(me: string): Table {
  const banner = document.querySelector('[data-slot="banner"]')?.textContent ?? ''
  const lives = Array.from(
    document.querySelectorAll<HTMLElement>('section[data-seat], button[data-tile-seat]'),
  )
    .map((pod) => {
      const seat = pod.dataset['seat'] ?? pod.dataset['tileSeat'] ?? '?'
      const life = /-?\d+/.exec(pod.querySelector('[data-slot="life"]')?.textContent ?? '')?.[0] ?? '?'
      return `${seat}:${life}`
    })
    .sort()
  return {
    turn: banner.replace(/^Your turn/, `${me}'s turn`),
    lives,
    log: document.querySelectorAll('[role="log"] li').length,
    over: document.querySelector('#over-title') !== null,
  }
}

/** Runs in the page: the tax and the commander damage chips as they are spoken. */
function chipsOf(): { tax: string[]; damage: string[] } {
  const words = (slot: string): string[] =>
    Array.from(document.querySelectorAll(`[data-slot="${slot}"]`)).map(
      (chip) => chip.getAttribute('aria-label') ?? '',
    )
  return { tax: words('commander-tax'), damage: words('commander-damage') }
}

/** "Emmara, Soul of the Accord: cast 2 times from the command zone, tax 4 mana" as numbers. */
function taxOf(words: string): { name: string; casts: number; tax: number } | null {
  const match = /^(.+): cast (\d+) times? from the command zone, tax (\d+) mana$/.exec(words)
  return match === null ? null : { name: match[1] ?? '', casts: Number(match[2]), tax: Number(match[3]) }
}

/** What the run saw of the commander rules, collected at every decision of every seat. */
class Seen4 {
  /** Every tax chip ever spoken, by commander: casts to tax. */
  readonly tax = new Map<string, Map<number, number>>()
  /** Every distinct commander damage chip spoken. */
  readonly damage = new Set<string>()
  /** Turns the seats were compared in. */
  readonly compared = new Set<number>()
  /** The least number of log lines a seat showed when compared after the first turn. */
  leastLog = Infinity
  /** The layout problems found, by the moment. */
  readonly layout: string[] = []
  private readonly looked = new Set<string>()
  /** The first turn a commander damage chip showed and the log of the seat that saw it. */
  firstDamage: { turn: number; log: string } | null = null
  highestTurn = 0

  private readonly players: readonly Player[]

  constructor(players: readonly Player[]) {
    this.players = players
  }

  /** Reads the chips on `player`'s page. */
  async chips(player: Player, turn: number): Promise<void> {
    const { tax, damage } = await player.page.evaluate(chipsOf)
    for (const words of tax) {
      const parsed = taxOf(words)
      if (parsed === null) continue
      const casts = this.tax.get(parsed.name) ?? new Map<number, number>()
      casts.set(parsed.casts, parsed.tax)
      this.tax.set(parsed.name, casts)
    }
    for (const words of damage) this.damage.add(words)
    if (damage.length > 0 && this.firstDamage === null) {
      const log = (await player.page.locator('[role="log"]').first().textContent()) ?? ''
      this.firstDamage = { turn, log }
    }
  }

  /**
   * Waits until every seat's page reads the same turn and the same lives. Called while one seat holds priority, so the
   * game stands still and the others only have to catch up.
   */
  async agree(turn: number): Promise<void> {
    if (this.compared.has(turn)) return
    this.compared.add(turn)
    const read = async (): Promise<Table[]> =>
      Promise.all(this.players.map((player) => player.page.evaluate(tableOf, player.name)))
    let tables: Table[] = []
    await expect
      .poll(
        async () => {
          tables = await read()
          if (tables.some((table) => table.over)) return 'over'
          const [head] = tables
          return tables.every(
            (table) => table.turn === head?.turn && table.lives.join() === head?.lives.join(),
          )
            ? 'same'
            : tables.map(
                (table, index) => `${this.players[index]?.name}: ${table.turn} ${table.lives.join(' ')}`,
              )
        },
        { message: `the four seats read different turns or lives at turn ${turn}`, timeout: 10_000 },
      )
      .toMatch(/^(same|over)$/)
    if (turn > 1) this.leastLog = Math.min(this.leastLog, ...tables.map((table) => table.log))
  }

  /** Holds the page of `player` to the layout rules once for each kind of decision: no scrolling, nothing cut off. */
  async layoutOnce(player: Player, seen: Seen): Promise<void> {
    const key = `${player.name}:${seen.kind}`
    if (this.looked.has(key)) return
    this.looked.add(key)
    const ask = { actions: {}, reveal: false, exempt: TABLE_EXEMPT, floor: TEXT_FLOOR }
    const problems = await settle(
      (): Promise<Look> => player.page.evaluate(measure, ask),
      { fits: true },
      { timeoutMs: 3_000 },
    )
    if (problems.length > 0)
      this.layout.push(`${player.name} at ${seen.kind} (${seen.banner}): ${problems.join('; ')}`)
  }
}

/** The headline and the rows of the finished-seats list of the game-over screen. */
async function finish(page: Page): Promise<{ headline: string; rows: string[] }> {
  const headline = (await page.getByRole('heading', { level: 1 }).textContent()) ?? ''
  const rows = await page
    .getByRole('list', { name: 'How each seat finished' })
    .getByRole('listitem')
    .allTextContents()
  return {
    headline,
    rows: rows.map((row) => row.replace(/\s+/g, ' ').replace(/you(Winner|Lost|Draw)/, '$1')),
  }
}

test.describe('four-seat Commander', () => {
  // A failed test restarts the worker and its beforeAll would register four more accounts (ten an address in 15 minutes).
  test.describe.configure({ mode: 'serial' })
  const accounts: Account[] = []
  let decks: PreconListItem[] = []

  test.beforeAll(async ({ browser }) => {
    for (const prefix of ['ada', 'bo', 'cy', 'di']) accounts.push(await signUp(browser, prefix))
    // Commanders of at most four mana: cast early, and again with the tax of {2}.
    decks = await smallCommanderPrecons(browser, 4, 4)
  })

  test('four precon decks play from the lobby to a winner', async ({ browser }, testInfo) => {
    test.setTimeout(15 * 60_000)
    expect(
      decks,
      'the database has fewer than four playable Commander precons with a small commander',
    ).toHaveLength(4)
    testInfo.annotations.push({ type: 'decks', description: decks.map((deck) => deck.code).join(', ') })
    const players = await startTable(browser, accounts, decks, HOUSE_RULES)
    const run = new Run(MAX_DECISIONS, STALL_MS)
    const seen = new Seen4(players)
    try {
      // The lobby names the house rules to every seat.
      for (const { page } of players)
        await expect(page.getByText(`Starting life ${HOUSE_RULES['Starting life']}`).first()).toBeVisible()
      await everyoneReady(players)

      const names = players.map((player) => player.name)
      const styles: Style[] = players.map((_, index) => ({
        // Nobody blocks and everyone pays a different way; every seat goes for the first player still in the game.
        blocks: false,
        autoPay: index % 2 === 0,
        yields: false,
        mulligans: true,
        aim: names,
        ...(RECALLING.includes(index) ? { recall: decks[index]?.commanders[0] ?? '' } : {}),
      }))
      const hooks: Hooks = {
        async before(player: Player, now: Seen, current: Run) {
          const turn = turnOf(now.banner)
          seen.highestTurn = Math.max(seen.highestTurn, turn)
          if (turn >= TURN_CAP) {
            current.done = true
            return
          }
          await seen.chips(player, turn)
          await seen.layoutOnce(player, now)
          if (now.kind === 'Priority') await seen.agree(turn)
        },
      }
      await playToTheEnd(
        players.map((player, index) => ({ player, style: styles[index] as Style })),
        run,
        hooks,
      )
      console.log(
        `four seats: ${decks.map((d) => d.code).join(', ')}: ${run.summary()}, last turn ${seen.highestTurn}`,
      )
      // Where a seat was asked a window with nothing to play in, for the cause (docs/ROADMAP.md, k3d).
      for (const [name, banners] of run.idleAt)
        console.log(`idle windows of ${name}: ${banners.slice(0, 6).join(' | ')}`)
      const taxes = [...seen.tax].map(
        ([name, casts]) => `${name}: ${[...casts].map((c) => c.join('→')).join(', ')}`,
      )
      console.log(`commander tax chips (casts→tax): ${taxes.join(' | ')}`)
      console.log(`commander damage chips: ${[...seen.damage].join(' | ')}`)
      expect(seen.highestTurn, `no winner by turn ${TURN_CAP}`).toBeLessThan(TURN_CAP)

      // Every seat is told the game is over, and they agree on who won and how each seat finished.
      for (const { page } of players) await expect(page.locator('#over-title')).toBeVisible()
      const ends = await Promise.all(players.map(({ page }) => finish(page)))
      expect(ends.map((end) => end.headline).sort(), 'one seat won, three lost').toEqual([
        'You lost',
        'You lost',
        'You lost',
        'You won',
      ])
      const [head] = ends
      for (const end of ends)
        expect(end.rows, 'the seats finished the same on every page').toEqual(head?.rows)
      expect(
        head?.rows.filter((row) => row.includes('Winner')),
        'exactly one Winner',
      ).toHaveLength(1)
      const gameOver = await Promise.all(
        players.map(({ page }) =>
          settle(
            (): Promise<Look> =>
              page.evaluate(measure, { actions: {}, reveal: false, exempt: [], floor: TEXT_FLOOR }),
            { fits: true },
          ),
        ),
      )
      expect(gameOver, 'the game-over screen scrolls or is cut off').toEqual([[], [], [], []])

      // The rules of the format showed: mulligans were decided, a commander was cast, then cast again with its tax.
      expect(run.kinds.get('Mulligan') ?? 0, 'every seat decided its opening hand').toBeGreaterThanOrEqual(4)
      expect(seen.tax.size, 'no commander was cast from the command zone').toBeGreaterThan(0)
      const twice = [...seen.tax.values()].filter((casts) => casts.has(2))
      expect(twice.length, 'no commander was cast a second time').toBeGreaterThan(0)
      for (const casts of twice) {
        expect(casts.get(1), 'the tax after one cast').toBe(2)
        expect(casts.get(2), 'the tax after two casts').toBe(4)
      }
      expect(seen.damage.size, 'no commander damage chip showed').toBeGreaterThan(0)
      expect(seen.firstDamage?.log ?? '', 'the log tells commander damage').toMatch(/commander damage/)
      expect(seen.leastLog, 'a seat showed an empty game log').toBeGreaterThan(0)
      expect(seen.layout, 'pages scrolled or were cut off').toEqual([])
      for (const player of players) {
        expect(player.errors, `${player.name}'s page errors`).toEqual([])
        expect(player.consoleErrors, `${player.name}'s console errors`).toEqual([])
      }
    } catch (error) {
      await attachFailure(testInfo, run, players)
      throw error
    } finally {
      if (process.env['E2E_TRAIL'] === '1') console.log(run.trail.join('\n'))
      await Promise.all(players.map(({ context }) => context.close()))
    }
  })
})
