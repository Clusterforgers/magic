// Goal: on a server with a clock on the active player's decisions (E2E_ACTIVE_SECS), a seat that finishes three turns of its own in time earns a clock extension that both seats see on its pod, and when it lets the clock run out on a later decision the extension is spent and both seats see "used" with the count down.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { playToTheEnd, Run } from './driver'
import type { Hooks, Style } from './driver'
import { createAndJoin, everyoneReady } from './lobby'
import type { Player } from './lobby'
import { ACTIVE_SECS } from './ports'

const MAX_DECISIONS = 2_500
const STALL_MS = 40_000

/** The seats that show a bank mark on this page, with each mark's count. */
const marks = (page: Page): Promise<Array<{ seat: number; bank: number }>> =>
  page.evaluate(() =>
    [...document.querySelectorAll('[data-slot="bank-mark"]')].flatMap((mark) => {
      const seat = mark.closest('[data-seat]')?.getAttribute('data-seat')
      return seat === null || seat === undefined
        ? []
        : [{ seat: Number(seat), bank: Number(mark.getAttribute('data-bank')) }]
    }),
  )

const mark = (player: Player, seat: number) =>
  player.page.locator(`[data-seat="${seat}"] [data-slot="bank-mark"]`)

test.skip(ACTIVE_SECS === undefined, 'needs a server with E2E_ACTIVE_SECS set')

test('an earned extension shows on the pod to both seats, and so does the one that is spent', async ({
  browser,
}, testInfo) => {
  test.setTimeout(5 * 60_000)
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  const run = new Run(MAX_DECISIONS, STALL_MS)
  let holder: number | undefined
  const hooks: Hooks = {
    async before(player, seen) {
      if (holder === undefined) {
        for (const { page } of players) {
          const [first] = await marks(page)
          if (first !== undefined) holder = first.seat
        }
        return
      }
      // The seat that holds the extension lets the clock of its own turn run out.
      const mine = players[holder]
      if (mine === undefined || player.name !== mine.name || !/^Your turn/.test(seen.banner)) return
      for (const watcher of players) {
        await expect(mark(watcher, holder), `${watcher.name} sees the extension used`).toHaveAttribute(
          'data-spent',
          'true',
          { timeout: 30_000 },
        )
        await expect(mark(watcher, holder)).toHaveAttribute('data-bank', '0')
      }
      run.done = true
    },
  }
  const style: Style = { blocks: false, autoPay: true, yields: true }
  try {
    await everyoneReady(players)
    await playToTheEnd(
      players.map((player) => ({ player, style })),
      run,
      hooks,
    )
    expect(holder, 'a seat earned an extension').toBeDefined()
    for (const { errors, name } of players) expect(errors, `${name}'s page errors`).toEqual([])
  } catch (error) {
    await testInfo.attach('trail.txt', { body: run.trail.join('\n') || '(no decision was answered)' })
    throw error
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
