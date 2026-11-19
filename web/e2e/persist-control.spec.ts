// Goal: the Full Control menu's settings and the Auto-skip triggers switch a player turns on stay on in their next game in the same browser: Full Control held, end turn by hand, every auto choice and the sounds, as the new game's server holds them.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { createAndJoin, everyoneReady } from './lobby'
import type { Player } from './lobby'

const fullControl = (page: Page, state: string) =>
  page.getByRole('button', { name: `Full Control: ${state}` })
const openMenu = (page: Page) => page.getByRole('button', { name: 'Full Control options' }).click()
const item = (page: Page, name: string) => page.getByRole('menuitemcheckbox', { name, exact: true })
const autoSkip = (page: Page) => page.getByRole('switch', { name: 'Auto-skip triggers' })

/** The menu's switches this spec turns on, each kept by the server or this browser. */
const TOGGLED = [
  'End my turn by hand',
  'Order my triggers by timestamp',
  'Assign combat damage for me',
  'Stop where my permanents can act',
  'Always pay automatically',
] as const

async function startGame(players: Player[]): Promise<void> {
  await everyoneReady(players)
  for (const { page } of players)
    await expect(fullControl(page, 'off').or(fullControl(page, 'held'))).toBeVisible()
}

test('the Full Control settings and Auto-skip triggers carry into the next game', async ({ browser }) => {
  test.setTimeout(3 * 60_000)
  const first = await createAndJoin(browser, [
    { name: 'Ada', deck: 'mono-green' },
    { name: 'Bo', deck: 'mono-red' },
  ])
  const [ada] = first
  if (ada === undefined) throw new Error('two players expected')
  let second: Player[] = []
  try {
    await startGame(first)
    const page = ada.page
    for (const name of TOGGLED) {
      await openMenu(page)
      await expect(item(page, name)).toHaveAttribute('aria-checked', 'false')
      await item(page, name).click()
      // The switch shows the server's word, so it is on once the server holds it.
      if ((await item(page, name).count()) === 0) await openMenu(page)
      await expect(item(page, name)).toHaveAttribute('aria-checked', 'true')
      await page.keyboard.press('Escape')
    }
    await openMenu(page)
    await item(page, 'Play sounds').click()
    if ((await item(page, 'Play sounds').count()) === 0) await openMenu(page)
    await expect(item(page, 'Play sounds')).toHaveAttribute('aria-checked', 'false')
    await page.getByRole('menuitemradio', { name: /^Held until I turn it off/ }).click()
    await expect(fullControl(page, 'held')).toHaveAttribute('aria-pressed', 'true')
    await autoSkip(page).click()
    await expect(autoSkip(page)).toBeChecked()

    // A new game in the same browser, with a new opponent: every choice is back without touching anything.
    second = await createAndJoin(
      browser,
      [
        { name: 'Ada', deck: 'mono-green' },
        { name: 'Cy', deck: 'mono-red' },
      ],
      ada.context,
    )
    const next = second[0]?.page
    if (next === undefined) throw new Error('two players expected')
    await startGame(second)
    await expect(fullControl(next, 'held')).toHaveAttribute('aria-pressed', 'true')
    await expect(autoSkip(next)).toBeChecked()
    await openMenu(next)
    for (const name of TOGGLED) await expect(item(next, name)).toHaveAttribute('aria-checked', 'true')
    await expect(item(next, 'Play sounds')).toHaveAttribute('aria-checked', 'false')
    await next.keyboard.press('Escape')

    // Turning Full Control off is kept too: a reload of the new game leaves it off.
    await fullControl(next, 'held').click()
    await expect(fullControl(next, 'off')).toHaveAttribute('aria-pressed', 'false')
    await next.reload()
    await expect(fullControl(next, 'off')).toHaveAttribute('aria-pressed', 'false')

    for (const player of [...first, ...second])
      expect(player.errors, `${player.name}'s page errors`).toEqual([])
  } finally {
    await Promise.all([...first, ...second.slice(1)].map(({ context }) => context.close()))
  }
})
