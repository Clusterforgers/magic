// Goal: in a real two-seat game, at desktop and phone size, one seat loses life, draws, mills, looks at the top and puts a card on the bottom, and makes a token from the Commands panel; each row says Done, both seats' logs tell every action, and the other seat's log never names the cards looked at.
import { expect, test } from '@playwright/test'
import type { Locator, Page } from '@playwright/test'
import { createAndJoin, everyoneReady } from './lobby'
import type { Player } from './lobby'

const SIZES = [
  { name: 'desktop', width: 1440, height: 900 },
  { name: 'phone', width: 390, height: 844 },
] as const

/** The side area: the column at desktop size, the sheet that the top bar's button opens on a phone. */
async function openSide(page: Page, tab: 'Commands' | 'Log'): Promise<void> {
  const button = page.getByRole('button', { name: /^Commands, log, chat and stack/ })
  if (await button.isVisible()) {
    if ((await button.getAttribute('aria-expanded')) !== 'true') await button.click()
  }
  await page.getByRole('tab', { name: new RegExp(`^${tab}`) }).click()
}

/** The panel's row of a command, opened. */
async function openRow(page: Page, name: string): Promise<Locator> {
  const panel = page.getByRole('region', { name: 'Commands' })
  const toggle = panel.locator('button[aria-expanded]', { hasText: new RegExp(`^${name}`) }).first()
  // The table redraws while a turn starts: look again until the row is open.
  await expect(async () => {
    if (!(await panel.isVisible())) await openSide(page, 'Commands')
    if ((await toggle.getAttribute('aria-expanded', { timeout: 2000 })) !== 'true')
      await toggle.click({ timeout: 2000 })
    await expect(toggle).toHaveAttribute('aria-expanded', 'true', { timeout: 1000 })
  }).toPass({ timeout: 20_000 })
  const row = panel.locator('[data-open]')
  await expect(row).toHaveCount(1)
  return row
}

/** Presses Apply, which names the change, and waits for the row to say the server took it. */
async function apply(row: Locator, label: string | RegExp): Promise<void> {
  const button = row.locator('[data-slot="command-apply"]')
  await expect(button).toHaveText(label)
  // The open row is scrolled into the panel's view, so Apply is in reach without a scroll.
  await expect(button).toBeInViewport()
  await button.click()
  await expect(row.locator('[data-slot="command-feedback"]')).toContainText(/^(Done|Sent)/)
}

/** The text of the seat's log, once it holds every line in `lines`. */
async function logOf(page: Page, lines: readonly RegExp[]): Promise<string> {
  await openSide(page, 'Log')
  const log = page.getByRole('log', { name: 'Game log' })
  for (const line of lines) await expect(log).toContainText(line)
  return (await log.textContent()) ?? ''
}

const BOTH = [
  /Ada loses 5 life \(by hand\)/,
  /Ada draws 2 cards \(by hand\)/,
  /Ada moves the top 2 cards of their library to the graveyard \(by hand\)/,
  /Ada creates 1 Treasure token \(by hand\)/,
]

for (const size of SIZES) {
  test(`the Commands panel acts for one seat and both logs tell it, at ${size.name} size`, async ({
    browser,
  }) => {
    test.setTimeout(3 * 60_000)
    const players = await createAndJoin(browser, [
      { name: 'Ada', deck: 'mono-green' },
      { name: 'Bo', deck: 'mono-red' },
    ])
    const [ada, bo] = players as [Player, Player]
    try {
      for (const { page } of players) await page.setViewportSize({ width: size.width, height: size.height })
      await everyoneReady(players)
      await Promise.all(
        players.map(({ page }) => page.getByRole('button', { name: 'Keep', exact: true }).click()),
      )
      // The table that follows the opening hands draws its own side area: open it once the hands are kept.
      for (const { page } of players) {
        await expect(page.getByRole('heading', { name: 'Opening hands' })).toHaveCount(0)
      }
      const page = ada.page
      // Ada has the first turn of the pinned seed.
      await expect(page.getByText('You have priority', { exact: false }).first()).toBeVisible()
      await openSide(page, 'Commands')
      await expect(page.getByRole('region', { name: 'Commands' })).toBeVisible()

      let row = await openRow(page, 'Life')
      await row.getByRole('button', { name: 'Take 5' }).click()
      await expect(row.locator('[data-slot="command-preview"]')).toContainText('→')
      await apply(row, 'Lose 5 life')

      row = await openRow(page, 'Draw')
      await row.getByRole('button', { name: '2', exact: true }).click()
      await apply(row, 'Draw 2 cards')

      row = await openRow(page, 'Mill')
      await row.getByRole('button', { name: '2', exact: true }).click()
      await expect(row.locator('[data-slot="command-lasting"]')).toBeVisible()
      await apply(row, /^Mill 2 cards/)

      row = await openRow(page, 'Look at the top')
      await row.getByRole('button', { name: '3', exact: true }).click()
      // Once the server took the look, the row turns into the list of the cards looked at.
      await expect(row.locator('[data-slot="command-apply"]')).toHaveText('Look at the top 3 cards')
      await row.locator('[data-slot="command-apply"]').click()
      const list = page.locator('[data-slot="look-arrange"]')
      await expect(list.getByRole('listitem')).toHaveCount(3)
      await list.getByRole('listitem').first().getByRole('button', { name: 'Bottom' }).click()
      await expect(list.locator('[data-slot="command-apply"]')).toHaveText('Put 1 on the bottom')
      await list.locator('[data-slot="command-apply"]').click()
      await expect(list).toHaveCount(0)

      row = await openRow(page, 'Tokens')
      await row.getByRole('combobox', { name: 'Token' }).selectOption('Treasure')
      await row.getByRole('button', { name: '1', exact: true }).click()
      await apply(row, 'Create 1 Treasure')

      // Ada's own log names the cards she looked at and where each went; Bo's says how many, and no names.
      const own = await logOf(page, [
        ...BOTH,
        /Ada looks at the top 3 cards of their library: .+ \(by hand\)/,
        /Ada puts back the top 3 cards of their library: .+ on top, .+ on the bottom \(by hand\)/,
      ])
      const looked = /looks at the top 3 cards of their library: (.+?) \(by hand\)/.exec(own)?.[1] ?? ''
      expect(looked).not.toBe('')
      const theirs = await logOf(bo.page, [
        ...BOTH,
        /Ada looks at the top 3 cards of their library \(by hand\)/,
        /Ada puts back the top 3 cards of their library: 2 on top, 1 on the bottom \(by hand\)/,
      ])
      expect(theirs).not.toContain(looked)
      expect(theirs).not.toMatch(/looks at the top 3 cards of their library:/)
      for (const player of players) expect(player.errors).toEqual([])
    } finally {
      for (const player of players) await player.context.close()
    }
  })
}
