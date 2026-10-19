import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { createAndJoin, everyoneReady } from './lobby'
import type { Player } from './lobby'

/** A stop button of the ladder by the start of its spoken name, such as "Your upkeep stop: off". */
const stop = (page: Page, name: string) => page.getByRole('button', { name: new RegExp(`^${name}`) })
/** Shows one side of the stops row: "Mine" for your own turns or "Theirs" for the opponents'. */
const side = (page: Page, name: 'Mine' | 'Theirs') =>
  page
    .getByRole('group', { name: 'Stops for' })
    .getByRole('button', { name: new RegExp(`^${name}`) })
    .click()
const fullControl = (page: Page, state: string) =>
  page.getByRole('button', { name: `Full Control: ${state}` })
const lit = (page: Page) => page.locator('[data-slot="stop"][aria-pressed="true"]')

/** Gives the page's keys back to the table: a pressed button keeps the focus, and keys then belong to it. */
const releaseFocus = (page: Page) =>
  page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur())

test('stops and Full Control are set in the browser, shown as the server holds them and kept across a reload', async ({
  browser,
}) => {
  test.setTimeout(2 * 60_000)
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  const [ada, bo] = players
  if (ada === undefined || bo === undefined) throw new Error('two players expected')
  try {
    await everyoneReady(players)
    for (const { page } of players) {
      await expect(page.getByRole('group', { name: 'Stops', exact: true })).toBeVisible()
      await expect(lit(page)).toHaveCount(0)
      await expect(fullControl(page, 'off')).toHaveAttribute('aria-pressed', 'false')
    }

    // A press sets a one-turn stop, shift and press a lasting one with its pin; each shows once the server says so.
    await side(ada.page, 'Mine')
    await stop(ada.page, 'Your upkeep stop: off').click()
    await expect(stop(ada.page, 'Your upkeep stop: on for one turn')).toHaveAttribute('aria-pressed', 'true')
    await side(ada.page, 'Theirs')
    await stop(ada.page, "Opponents' end step stop: off").click({ modifiers: ['Shift'] })
    const pinned = stop(ada.page, "Opponents' end step stop: on every turn")
    await expect(pinned).toHaveAttribute('data-keep', 'lasting')
    await expect(pinned.locator('.lucide-pin')).toBeVisible()
    await expect(lit(ada.page)).toHaveCount(2)

    // The stops are Ada's alone.
    await expect(lit(bo.page)).toHaveCount(0)

    // Another press clears a stop; the Keep toggle makes a plain press set a lasting one.
    await side(ada.page, 'Mine')
    await stop(ada.page, 'Your upkeep stop: on for one turn').click()
    await expect(lit(ada.page)).toHaveCount(1)
    await ada.page.getByRole('button', { name: 'Keep stops' }).click()
    await stop(ada.page, 'Your declare attackers stop: off').click()
    await expect(stop(ada.page, 'Your declare attackers stop: on every turn')).toHaveAttribute(
      'aria-pressed',
      'true',
    )
    await ada.page.getByRole('button', { name: 'Keep stops' }).click()

    // The keys: Control asks for the next window (and again takes it back), Control with Shift for Full Control held.
    await releaseFocus(ada.page)
    await ada.page.keyboard.press('Control')
    await expect(fullControl(ada.page, 'next window')).toHaveAttribute('aria-pressed', 'true')
    await ada.page.keyboard.press('Control')
    await expect(fullControl(ada.page, 'off')).toHaveAttribute('aria-pressed', 'false')
    await ada.page.keyboard.press('Control+Shift')
    await expect(fullControl(ada.page, 'held')).toHaveAttribute('aria-pressed', 'true')
    await expect(fullControl(bo.page, 'off')).toBeVisible()

    // The menu asks for a mode by name, and the button lets go of a held one.
    await ada.page.getByRole('button', { name: 'Full Control options' }).click()
    await ada.page.getByRole('menuitemradio', { name: /^Next window only/ }).click()
    await expect(fullControl(ada.page, 'next window')).toBeVisible()
    await ada.page.getByRole('button', { name: 'Full Control options' }).click()
    await ada.page.getByRole('menuitemradio', { name: /^Held until I turn it off/ }).click()
    await expect(fullControl(ada.page, 'held')).toBeVisible()

    // The server keeps both for the seat: a reload joins again and the Welcome brings them back.
    await ada.page.reload()
    await expect(fullControl(ada.page, 'held')).toHaveAttribute('aria-pressed', 'true')
    await expect(lit(ada.page)).toHaveCount(2)
    await side(ada.page, 'Theirs')
    await expect(stop(ada.page, "Opponents' end step stop: on every turn")).toBeVisible()
    await side(ada.page, 'Mine')
    await expect(stop(ada.page, 'Your declare attackers stop: on every turn')).toBeVisible()
    await fullControl(ada.page, 'held').click()
    await expect(fullControl(ada.page, 'off')).toHaveAttribute('aria-pressed', 'false')

    for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})

/**
 * Starts a game, has both seats keep their hands and waits for the first priority window the game asks of anybody: the
 * seat that has a land to play first. Returns the page of that seat with the id of its decision.
 */
async function firstWindow(players: Player[]): Promise<{ page: Page; id: string }> {
  await everyoneReady(players)
  // The first player decides first and the other's Keep appears when that one has pressed it, so both are waited for.
  await Promise.all(
    players.map(({ page }) => page.getByRole('button', { name: 'Keep', exact: true }).click()),
  )
  const prompts = players.map(({ page }) => page.locator('[data-decision="Priority"]'))
  let seat = -1
  await expect
    .poll(
      async () => {
        for (const [index, prompt] of prompts.entries()) {
          if ((await prompt.count()) > 0) seat = index
        }
        return seat
      },
      { timeout: 45_000 },
    )
    .toBeGreaterThanOrEqual(0)
  const player = players[seat]
  const prompt = prompts[seat]
  if (player === undefined || prompt === undefined) throw new Error('nobody was asked')
  const id = await prompt.getAttribute('data-decision-id')
  if (id === null) throw new Error('the prompt has no decision id')
  await releaseFocus(player.page)
  return { page: player.page, id }
}

const decisionWindow = (page: Page, id: string) =>
  page.locator(`[data-decision="Priority"][data-decision-id="${id}"]`)

test('Enter ends the turn from the keyboard of a real page', async ({ browser }) => {
  test.setTimeout(2 * 60_000)
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  try {
    const { page, id } = await firstWindow(players)
    // Enter asks the server to pass until the end of the turn. With nothing for anyone to do the yield can be over
    // within the one step the server takes, so the page may never say it is passing; the window is gone either way.
    await page.keyboard.press('Enter')
    await expect(decisionWindow(page, id)).toHaveCount(0)
    for (const each of players) expect(each.errors, `${each.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})

test('Space passes once from the keyboard of a real page', async ({ browser }) => {
  test.setTimeout(2 * 60_000)
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  try {
    const { page, id } = await firstWindow(players)
    // Space answers the decision on screen with a pass, and it gives way to what comes next; no yield starts.
    await page.keyboard.press('Space')
    await expect(decisionWindow(page, id)).toHaveCount(0)
    await expect(page.getByText('Passing until the end of turn')).toHaveCount(0)
    for (const each of players) expect(each.errors, `${each.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
