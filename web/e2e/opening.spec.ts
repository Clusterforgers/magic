// Goal: two seats create a game, join by link and see in each other's lobby, as it happens, who is here and who has pressed Ready (and that Ready can be taken back and survives a reload), and once both are ready they are dealt opening hands and the table lists each seat's opening-hand status.
import { expect, test } from '@playwright/test'
import { createAndJoin, everyoneReady, seatRows } from './lobby'

test('two seats create, join by link and are dealt opening hands', async ({ browser }) => {
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  try {
    await everyoneReady(players)
    for (const { name, page } of players) {
      await expect(page.getByRole('heading', { name: 'Opening hands' })).toBeVisible()
      await expect(page.getByRole('list', { name: `${name}'s hand` }).getByRole('listitem')).toHaveCount(7)
      // Every seat's public status is on the table: this seat as "You", the other by name.
      const status = page.getByRole('region', { name: 'Opening hand decisions' }).getByRole('listitem')
      await expect(status).toHaveCount(2)
      const other = name === 'Ada' ? 'Bo' : 'Ada'
      await expect(status.filter({ hasText: 'You:' })).toHaveCount(1)
      await expect(status.filter({ hasText: `${other}:` })).toHaveCount(1)
    }
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})

test('the lobby shows who has pressed Ready as it happens, and Ready can be taken back', async ({
  browser,
}) => {
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  const [ada, bo] = players
  if (ada === undefined || bo === undefined) throw new Error('two players expected')
  const summary = (page: typeof ada.page) => page.locator('[data-slot="lobby-summary"]')
  try {
    // Both are here and neither is ready; each page marks its own seat.
    for (const { page } of players) {
      await expect(seatRows(page).nth(0)).toHaveAttribute('data-state', 'waiting')
      await expect(seatRows(page).nth(1)).toHaveAttribute('data-state', 'waiting')
    }
    await expect(seatRows(ada.page).nth(0)).toContainText('you')
    await expect(seatRows(bo.page).nth(1)).toContainText('you')
    await expect(summary(ada.page)).toHaveText('0 of 2 ready, waiting for you and Bo to press Ready')

    // Bo presses Ready: Ada's page shows it without a reload, and says who is being waited for.
    await bo.page.getByRole('button', { name: 'Ready', exact: true }).click()
    await expect(bo.page.getByRole('button', { name: 'Not ready', exact: true })).toBeVisible()
    await expect(seatRows(ada.page).nth(1)).toHaveAttribute('data-state', 'ready')
    await expect(seatRows(ada.page).nth(1)).toContainText('Ready')
    await expect(summary(ada.page)).toHaveText('1 of 2 ready, waiting for you to press Ready')
    await expect(summary(bo.page)).toHaveText('1 of 2 ready, waiting for Ada to press Ready')
    // The same sentence, said aloud once: the polite live region holds what changed.
    await expect(ada.page.locator('[data-slot="lobby-announcement"]')).toHaveText(
      'Bo is ready. 1 of 2 ready, waiting for you to press Ready',
    )

    // A reload brings the page back with what the server holds, before and after its socket opens.
    await bo.page.reload()
    await expect(bo.page.getByRole('button', { name: 'Not ready', exact: true })).toBeVisible()
    await expect(seatRows(bo.page).nth(1)).toHaveAttribute('data-state', 'ready')

    // Bo takes it back and Ada sees that too; nothing started.
    await bo.page.getByRole('button', { name: 'Not ready', exact: true }).click()
    await expect(bo.page.getByRole('button', { name: 'Ready', exact: true })).toBeVisible()
    await expect(seatRows(ada.page).nth(1)).toHaveAttribute('data-state', 'waiting')
    await expect(summary(ada.page)).toHaveText('0 of 2 ready, waiting for you and Bo to press Ready')
    await expect(ada.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()

    // Ada is ready, then Bo is: the last Ready starts the game for both.
    await ada.page.getByRole('button', { name: 'Ready', exact: true }).click()
    await expect(seatRows(bo.page).nth(0)).toHaveAttribute('data-state', 'ready')
    await bo.page.getByRole('button', { name: 'Ready', exact: true }).click()
    for (const { page } of players) {
      await expect(page.getByRole('heading', { name: 'Opening hands' })).toBeVisible()
    }
    for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})

test('four seats are each asked to keep or mulligan within a second of the start', async ({ browser }) => {
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
    { name: 'Cy', deck: 'mono-green' },
    { name: 'Dee', deck: 'mono-red' },
  ])
  const [ada] = players
  const last = players.at(-1)
  if (ada === undefined || last === undefined) throw new Error('four players expected')
  try {
    for (const { page } of players.slice(0, -1)) {
      await page.getByRole('button', { name: 'Ready', exact: true }).click()
      await expect(page.getByRole('button', { name: 'Not ready', exact: true })).toBeVisible()
    }
    const start = Date.now()
    await last.page.getByRole('button', { name: 'Ready', exact: true }).click()
    // Every seat is asked at once: no seat waits for the answers or the clocks of the others.
    const after = await Promise.all(
      players.map(async ({ page }) => {
        await expect(page.getByRole('button', { name: 'Keep', exact: true })).toBeVisible({ timeout: 5_000 })
        return Date.now() - start
      }),
    )
    for (const ms of after) expect(ms).toBeLessThan(1_000)
    // One seat keeps; the others keep their own prompt and read that seat as keeping.
    await ada.page.getByRole('button', { name: 'Keep', exact: true }).click()
    for (const { page } of players.slice(1)) {
      await expect(page.locator('[data-seat-status="0"]')).toContainText('Ada: keeping')
      await expect(page.getByRole('button', { name: 'Keep', exact: true })).toBeVisible()
    }
    for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
