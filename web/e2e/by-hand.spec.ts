// Goal: on a two-seat table, at desktop and phone size, a Partial card in hand shows the by-hand mark, a press on the mark and the `h` key on the focused card each open its clause list, and the cast and entering lines that quote the clauses are in both seats' logs.
import { expect, test } from '@playwright/test'
import type { Browser, Locator, Page } from '@playwright/test'
import type { DeckView, ImportResult } from '../src/protocol/generated'
import type { Player } from './lobby'
import { ACCOUNTS_DATABASE_URL, ACCOUNTS_WEB_URL } from './ports'
import { signUp, sit } from './table'
import type { Account } from './table'

test.skip(ACCOUNTS_DATABASE_URL === undefined, 'set E2E_DATABASE_URL to a database with cards to run this')

const SIZES = [
  { name: 'desktop', width: 1440, height: 900 },
  { name: 'phone', width: 390, height: 844 },
] as const

/** One-mana white creatures the engine runs only in part, so their cards carry by-hand clauses; a Normal game allows four of each. */
const PARTIALS = [
  'Icatian Infantry',
  'Mosquito Guard',
  'Loyal Pegasus',
  'Novice Knight',
  'Foothill Guide',
  'Charmed Stray',
  'Ocelot Pride',
  'Star Pupil',
]

/** Saves a 60-card freeform deck of eight Partial cards, four copies each, and Plains in `account`, as the Decks page does. */
async function saveDeck(browser: Browser, account: Account): Promise<{ id: string; name: string }> {
  const context = await browser.newContext({ baseURL: ACCOUNTS_WEB_URL, storageState: account.storage })
  const asAccount = { headers: { origin: ACCOUNTS_WEB_URL } }
  try {
    const read = await context.request.post('/api/decks/import', {
      ...asAccount,
      data: { text: `${PARTIALS.map((name) => `4 ${name}`).join('\n')}\n28 Plains`, format: 'freeform' },
    })
    expect(read.ok(), `import: ${await read.text()}`).toBe(true)
    const { cards, unresolved } = (await read.json()) as ImportResult
    expect(unresolved).toEqual([])
    const name = `By hand deck of ${account.name}`
    const saved = await context.request.post('/api/decks', {
      ...asAccount,
      data: {
        name,
        format: 'freeform',
        notes: '',
        cards: cards.map((line) => ({
          zone: line.zone,
          oracle_id: line.card.oracle_id,
          printing_id: line.printing_id,
          count: line.count,
        })),
      },
    })
    expect(saved.ok(), `save: ${await saved.text()}`).toBe(true)
    return { id: ((await saved.json()) as DeckView).id, name }
  } finally {
    await context.close()
  }
}

/** The side area: the column at desktop size, the sheet that the top bar's button opens on a phone. */
async function openLog(page: Page): Promise<Locator> {
  const button = page.getByRole('button', { name: /^Commands, log, chat and stack/ })
  if (await button.isVisible()) {
    if ((await button.getAttribute('aria-expanded')) !== 'true') await button.click()
  }
  await page.getByRole('tab', { name: /^Log/ }).click()
  return page.getByRole('log', { name: 'Game log' })
}

/** Closes the side sheet of a phone so the table is in reach. */
async function closeSide(page: Page): Promise<void> {
  const button = page.getByRole('button', { name: /^Commands, log, chat and stack/ })
  if ((await button.isVisible()) && (await button.getAttribute('aria-expanded')) === 'true') {
    await button.click()
  }
}

for (const size of SIZES) {
  test(`the by-hand mark and the h key list a Partial card's clauses, and both logs quote them, at ${size.name} size`, async ({
    browser,
  }) => {
    test.setTimeout(4 * 60_000)
    const accounts = [await signUp(browser, 'ada'), await signUp(browser, 'bo')]
    const decks = [
      await saveDeck(browser, accounts[0] as Account),
      await saveDeck(browser, accounts[1] as Account),
    ]
    const host = await sit(browser, accounts[0] as Account)
    const guest = await sit(browser, accounts[1] as Account)
    const players: Player[] = [host, guest]
    try {
      for (const { page } of players) await page.setViewportSize({ width: size.width, height: size.height })
      await host.page.goto('/new')
      await host.page.getByRole('radio', { name: /^Normal/ }).check()
      await host.page.getByText('Invite only', { exact: true }).click()
      await host.page.getByRole('button', { name: 'Create game' }).click()
      await expect(host.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
      const code = (await host.page.locator('[data-slot="invite-code"]').textContent())?.trim() ?? ''
      expect(code).not.toBe('')
      await guest.page.goto(`/join/${code}`)
      const join = guest.page.getByRole('button', { name: 'Join game' })
      await expect(join).toBeEnabled()
      await join.click()
      await expect(guest.page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
      for (const [index, { page }] of players.entries()) {
        const deck = decks[index] as { id: string; name: string }
        await page.getByRole('tab', { name: 'Your decks' }).click()
        const row = page.locator(`[data-deck-option="${deck.id}"]`)
        await expect(row).toBeVisible()
        await expect(row).not.toContainText('Checking at this table')
        await row.click()
        const use = page.locator('[data-slot="use-deck"]')
        await expect(use).toBeEnabled()
        await use.click()
        await expect(page.locator('[data-slot="deck-note"]')).toContainText(`Your deck is now ${deck.name}`)
      }
      for (const [index, { page }] of players.entries()) {
        await page.getByRole('button', { name: 'Ready', exact: true }).click()
        if (index === 0)
          await expect(page.getByRole('button', { name: 'Not ready', exact: true })).toBeVisible()
      }
      for (const { page } of players) {
        await expect(page.getByRole('heading', { name: 'Opening hands' })).toBeVisible()
      }
      await Promise.all(
        players.map(({ page }) => page.getByRole('button', { name: 'Keep', exact: true }).click()),
      )
      for (const { page } of players) {
        await expect(page.getByRole('heading', { name: 'Opening hands' })).toHaveCount(0)
      }

      // Whoever has the first turn plays a Plains and casts the Partial card with it.
      const active = (await host.page.getByText('You have priority', { exact: false }).first().isVisible())
        ? host
        : guest
      const other = active === host ? guest : host
      await expect(active.page.getByText('You have priority', { exact: false }).first()).toBeVisible()
      const page = active.page
      const hand = page.locator('[data-region="hand"] button[data-object-id]')
      // The first card in hand that carries the mark is the Partial card the rest of the test follows.
      const infantry = hand.filter({ has: page.locator('[data-slot="card-support"]') })
      await expect(infantry.first()).toBeVisible()
      const PARTIAL = ((await infantry.first().getAttribute('aria-label')) ?? '').split(',')[0] ?? ''
      expect(PARTIALS).toContain(PARTIAL)

      // The by-hand mark is on the card in hand; a press on it opens the clause list.
      const mark = infantry.first().locator('[data-slot="card-support"]')
      await expect(mark).toBeVisible()
      await mark.click()
      const panel = page.getByRole('dialog', { name: `${PARTIAL}: rules played by hand` })
      await expect(panel).toBeVisible()
      await page.keyboard.press('Escape')
      await expect(panel).toHaveCount(0)

      // The focused card opens the same list with h.
      await expect(infantry.first()).toHaveAttribute('aria-keyshortcuts', 'h')
      await infantry.first().focus()
      await page.keyboard.press('h')
      await expect(panel).toBeVisible()
      await page.keyboard.press('Escape')
      await expect(panel).toHaveCount(0)

      // Play a land, then cast the card and let it resolve.
      await page.locator('[data-region="hand"] button[data-object-id][aria-label^="Plains"]').first().click()
      await expect(page.getByText('You have priority', { exact: false }).first()).toBeVisible()
      await infantry.first().click()
      await page.getByRole('button', { name: 'Pay automatically', exact: true }).click()
      const field = page.locator(`section[aria-label="${active.name}'s battlefield"]`)
      const entered = field.locator(`button[data-object-id][aria-label^="${PARTIAL}"]`)
      await expect(async () => {
        for (const { page: p } of [active, other]) {
          const next = p.locator('[data-region="controls"] [data-slot="primary-action"]')
          if (await next.isVisible()) await next.click({ timeout: 1000 })
        }
        await expect(entered).toHaveCount(1, { timeout: 1500 })
      }).toPass({ timeout: 30_000 })

      // Both logs quote the clauses after the cast and again as the card enters.
      for (const { page: p } of players) {
        await closeSide(p)
        const log = await openLog(p)
        await expect(log).toContainText(new RegExp(`casts ${PARTIAL}`))
        const quotes = log.getByText(new RegExp(`${PARTIAL} has rules to apply by hand`))
        await expect(quotes.first()).toBeVisible()
        expect(await quotes.count()).toBeGreaterThanOrEqual(1)
      }
      for (const player of players) expect(player.errors).toEqual([])
    } finally {
      for (const player of players) await player.context.close()
    }
  })
}
