import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { playToTheEnd, Run } from './driver'
import { createAndJoin, everyoneReady } from './lobby'

/** Decisions both seats may answer before the run is called stuck; a vanilla game takes a few hundred. */
const MAX_DECISIONS = 2_500
/** Time with no decision answered by either seat before the run is called stuck. */
const STALL_MS = 30_000

const standings = (page: Page) => page.getByRole('list', { name: 'How each seat finished' })

/** The name of the switch in the table's toolbar that asks the server to pass for a seat with nothing to do. */
const SKIP = "Skip turns when I can't act"

test('two seats play a vanilla game to the end in the browser', async ({ browser }, testInfo) => {
  test.setTimeout(5 * 60_000)
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  const [ada, bo] = players
  if (ada === undefined || bo === undefined) throw new Error('two players expected')
  const run = new Run(MAX_DECISIONS, STALL_MS)
  try {
    await everyoneReady(players)
    // Both seats start with the server's default: passing for a seat that has nothing to do is on. Bo turns it off, so
    // the two settings meet in one game: Ada is never asked about a window she cannot use, Bo is asked about all.
    for (const { page } of players) await expect(page.getByRole('switch', { name: SKIP })).toBeChecked()
    await bo.page.getByRole('switch', { name: SKIP }).click()
    await expect(bo.page.getByRole('switch', { name: SKIP })).not.toBeChecked()
    // Ada taps her lands by hand and never blocks; Bo pays automatically and blocks, so each path is walked by a seat.
    await playToTheEnd(
      [
        { player: ada, style: { blocks: false, autoPay: false } },
        { player: bo, style: { blocks: true, autoPay: true } },
      ],
      run,
    )
    expect(run.idle.get('Ada') ?? 0, 'Ada was asked about windows she could do nothing in').toBe(0)
    expect(run.idle.get('Bo') ?? 0, 'Bo asked to be stopped at every window').toBeGreaterThan(0)

    // Both screens agree on the winner: one seat is told it won, the other that it lost, and each list names one Winner.
    const headlines = await Promise.all(
      players.map(({ page }) => page.getByRole('heading', { level: 1 }).textContent()),
    )
    expect([...headlines].sort()).toEqual(['You lost', 'You won'])
    const winner = players[headlines.indexOf('You won')]
    if (winner === undefined) throw new Error('nobody won')
    for (const { page } of players) {
      const list = standings(page)
      await expect(list.getByRole('listitem')).toHaveCount(2)
      await expect(list.getByRole('listitem').filter({ hasText: 'Winner' })).toHaveCount(1)
      await expect(list.getByRole('listitem').filter({ hasText: winner.name })).toContainText('Winner')
    }
    for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } catch (error) {
    await testInfo.attach('trail.txt', { body: run.trail.join('\n') || '(no decision was answered)' })
    for (const { name, page } of players) {
      await testInfo.attach(`${name}.png`, { body: await page.screenshot(), contentType: 'image/png' })
    }
    throw error
  } finally {
    testInfo.annotations.push({ type: 'decisions', description: String(run.steps) })
    console.log(`full game: ${run.summary()}`)
    if (process.env.E2E_TRAIL === '1') console.log(run.trail.join('\n'))
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
