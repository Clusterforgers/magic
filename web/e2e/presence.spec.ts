// Goal: when a seat's page closes mid-game the other seat sees it away within a few seconds, on its pod and in the "Waiting for" line with the time before the game acts for it, and the mark clears once the seat opens its page again.
import { expect, test } from '@playwright/test'
import { createAndJoin, everyoneReady } from './lobby'
import { SHOTS_DIR } from './table'

const KEEP = { name: 'Keep', exact: true } as const

test('a seat whose page closes shows as away to the others until it comes back', async ({ browser }) => {
  test.setTimeout(90_000)
  const players = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  const [ada, bo] = players
  if (ada === undefined || bo === undefined) throw new Error('two seats expected')
  try {
    await everyoneReady(players)
    // Ada keeps, so the game waits for Bo alone and her table shows the line that says so.
    await ada.page.getByRole('button', KEEP).click()
    await expect(ada.page.locator('[data-slot="decision-line"]')).toContainText('Waiting for Bo')
    const boPod = ada.page.locator('[data-seat="1"]')
    await expect(boPod).not.toHaveAttribute('data-away')

    const boUrl = bo.page.url()
    await bo.page.close()
    await expect(boPod).toHaveAttribute('data-away', 'true', { timeout: 5_000 })
    await expect(boPod.locator('[data-slot="away-mark"]')).toHaveText('Away')
    const note = ada.page.locator('[data-slot="away-note"]')
    await expect(note).toContainText(/Bo is away: the game acts for them in \d+ s/)
    if (SHOTS_DIR !== undefined) await ada.page.screenshot({ path: `${SHOTS_DIR}/presence-away.png` })

    // Bo opens the page again: the mark and the note go, and Bo is asked about the hand where it was.
    bo.page = await bo.context.newPage()
    await bo.page.goto(boUrl)
    await expect(bo.page.getByRole('button', KEEP)).toBeVisible({ timeout: 10_000 })
    await expect(boPod).not.toHaveAttribute('data-away', { timeout: 5_000 })
    await expect(note).toHaveCount(0)
    expect(ada.errors, "Ada's page errors").toEqual([])
  } finally {
    await Promise.all(players.map(({ context }) => context.close()))
  }
})
