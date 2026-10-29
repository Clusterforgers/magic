// Goal: accounts play real precon decks from a Postgres database through the real UI, from the lobby into the first turns (mulligans, lands, spells with their targets and payments, attacks, every step), and the run records what stops a first real game.
import { expect, test } from '@playwright/test'
import type { PreconListItem } from '../src/protocol/generated'
import { playToTheEnd, Run, turnOf } from './driver'
import type { Hooks, Seen, Style } from './driver'
import { everyoneReady } from './lobby'
import type { Player } from './lobby'
import { ACCOUNTS_DATABASE_URL } from './ports'
import { attachFailure, precons, Shots, signUp, startTable, SHOTS_DIR } from './table'
import type { Account } from './table'

// Needs Postgres with the card data (`mtg-cli ingest` and `compile`) and the precons: set E2E_DATABASE_URL to such a
// database (docs/web.md says how). Without it `pnpm e2e` has no database and this file is skipped.
test.skip(
  ACCOUNTS_DATABASE_URL === undefined,
  'set E2E_DATABASE_URL to a database with cards and precons to run this',
)

/** Decisions all seats may answer before the run is called stuck. */
const MAX_DECISIONS = 1_500
/** Time with no decision answered by any seat before the run is called stuck. */
const STALL_MS = 25_000

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
            // The second seat pays by hand: the prompt offers only sources that can finish the payment.
            { blocks: false, autoPay: false, yields: false, mulligans: true },
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
        // Every other seat pays by hand, so mana rocks and lands of two colours are tapped through the prompt too.
        [
          { blocks: true, autoPay: true, yields: false, mulligans: true },
          { blocks: false, autoPay: false, yields: false, mulligans: true },
        ],
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
