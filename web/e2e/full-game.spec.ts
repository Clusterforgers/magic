import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { playToTheEnd, Run } from './driver'
import { createAndJoin, everyoneReady } from './lobby'

/** Decisions both seats may answer before the run is called stuck; a vanilla game takes a few hundred. */
const MAX_DECISIONS = 2_500
/** Time with no decision answered by either seat before the run is called stuck. */
const STALL_MS = 30_000
/** The stops the server sets for every seat: its own declare attackers and postcombat main, as the banner names them. */
const DEFAULT_STOP = /^Your turn · \d+ · (Declare attackers|Postcombat main)$/
/** The stop Bo sets: the end step of Ada's turns. */
const BO_STOP = /^Ada's turn · \d+ · End step$/

const standings = (page: Page) => page.getByRole('list', { name: 'How each seat finished' })

/** The Full Control button of a page, by the state it says ("off", "next window", "held"). */
const stop = (page: Page, name: string) => page.getByRole('button', { name: new RegExp(`^${name}`) })
/** Shows one side of the stops row: "Mine" for your own turns or "Theirs" for the opponents'. */
const side = (page: Page, name: 'Mine' | 'Theirs') =>
  page
    .getByRole('group', { name: 'Stops for' })
    .getByRole('button', { name: new RegExp(`^${name}`) })
    .click()
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
    // Both seats start with the server's defaults: Full Control is off and their own declare attackers and postcombat
    // main are pinned, already pressable during the opening hands.
    for (const { page } of players) {
      await expect(fullControl(page, 'off')).toHaveAttribute('aria-pressed', 'false')
      for (const step of ['declare attackers', 'postcombat main'])
        await expect(stop(page, `Your ${step} stop: on every turn`)).toHaveAttribute('data-keep', 'lasting')
    }
    // Bo holds Full Control and lets it go again; each press shows only once the server has said so, and Ada's page
    // hears nothing of it.
    await fullControl(bo.page, 'off').click()
    await expect(fullControl(bo.page, 'held')).toHaveAttribute('aria-pressed', 'true')
    await expect(fullControl(ada.page, 'off')).toBeVisible()
    await fullControl(bo.page, 'held').click()
    await expect(fullControl(bo.page, 'off')).toHaveAttribute('aria-pressed', 'false')
    // Bo stops at the end step of every opponent turn (shift for a lasting stop). A server that gives a stop its window
    // asks Bo there with nothing to do, and he ends the turn; one that does not yet never asks.
    await side(bo.page, 'Theirs')
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
    // Ada set no stop of her own and kept Full Control off, so a window with nothing to do in asks her only at the
    // default stops of her own turn.
    for (const where of run.idleAt.get('Ada') ?? []) expect(where).toMatch(DEFAULT_STOP)
    expect(run.yields.get('Ada') ?? 0).toBe(0)
    // Bo is asked about nothing but the stop he set, the end step of Ada's turn, and the defaults of his own, and ends
    // the turn in each of them.
    const boAt = run.idleAt.get('Bo') ?? []
    for (const where of boAt) expect(where).toMatch(new RegExp(`${BO_STOP.source}|${DEFAULT_STOP.source}`))
    expect(
      boAt.some((where) => BO_STOP.test(where)),
      'Bo was never asked at his stop',
    ).toBe(true)
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
