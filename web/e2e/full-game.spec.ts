import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { playToTheEnd, Run } from './driver'
import { createAndJoin, everyoneReady } from './lobby'

/** Decisions both seats may answer before the run is called stuck; a vanilla game takes a few hundred. */
const MAX_DECISIONS = 2_500
/** Time with no decision answered by either seat before the run is called stuck. */
const STALL_MS = 30_000

const standings = (page: Page) => page.getByRole('list', { name: 'How each seat finished' })

/** The Full Control button of a page, by the state it says ("off", "next window", "held"). */
const stop = (page: Page, name: string) => page.getByRole('button', { name: new RegExp(`^${name}`) })
const fullControl = (page: Page, state: string) =>
  page.getByRole('button', { name: `Full Control: ${state}` })

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
    // Both seats start with the server's defaults: Full Control is off.
    for (const { page } of players)
      await expect(fullControl(page, 'off')).toHaveAttribute('aria-pressed', 'false')
    // Bo holds Full Control and lets it go again; each press shows only once the server has said so, and Ada's page
    // hears nothing of it.
    await fullControl(bo.page, 'off').click()
    await expect(fullControl(bo.page, 'held')).toHaveAttribute('aria-pressed', 'true')
    await expect(fullControl(ada.page, 'off')).toBeVisible()
    await fullControl(bo.page, 'held').click()
    await expect(fullControl(bo.page, 'off')).toHaveAttribute('aria-pressed', 'false')
    // Bo stops at the end step of every opponent turn (shift for a lasting stop). A server that gives a stop its window
    // asks Bo there with nothing to do, and he ends the turn; one that does not yet never asks.
    await stop(bo.page, "Opponents' end step stop: off").click({ modifiers: ['Shift'] })
    await expect(stop(bo.page, "Opponents' end step stop: on every turn")).toHaveAttribute(
      'data-keep',
      'lasting',
    )
    // Ada taps her lands by hand and never blocks; Bo pays automatically and blocks, so each path is walked by a seat.
    await playToTheEnd(
      [
        { player: ada, style: { blocks: false, autoPay: false, yields: false } },
        { player: bo, style: { blocks: true, autoPay: true, yields: true } },
      ],
      run,
    )
    // Ada set no stop and kept Full Control off, so she is never asked about a window there is nothing to do in.
    expect(run.idle.get('Ada') ?? 0, 'Ada was asked about windows there was nothing to do in').toBe(0)
    expect(run.yields.get('Ada') ?? 0).toBe(0)
    // Bo is asked about nothing but the stop he set, the end step of Ada's turn, and ends the turn in each of them.
    for (const where of run.idleAt.get('Bo') ?? []) expect(where).toMatch(/ · Ada · End step$/)
    expect(run.yields.get('Bo') ?? 0, 'Bo did not end the turn in every window he was asked about').toBe(
      run.idle.get('Bo') ?? 0,
    )

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
