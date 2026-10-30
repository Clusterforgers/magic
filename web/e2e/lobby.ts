import { expect } from '@playwright/test'
import type { Browser, BrowserContext, Page } from '@playwright/test'

/** One player at the table: their own browser context, so their seat token and storage are not shared. */
export interface Player {
  name: string
  page: Page
  context: BrowserContext
  /** Uncaught exceptions in the page so far; the UI should never throw, so the driver fails on the first. */
  errors: string[]
  /** The page's console errors other than failed loads, for the specs that look; unset where not collected. */
  consoleErrors?: string[]
}

/** What a seat asks for in the create form: a name and the id of a built-in deck. */
export interface SeatPlan {
  name: string
  deck: string
}

/** A player in a new context, with the page's uncaught exceptions collected. */
async function newPlayer(browser: Browser, name: string): Promise<Player> {
  const context = await browser.newContext()
  const page = await context.newPage()
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  return { name, page, context, errors }
}

/**
 * Creates a game through the form on `/` as the first seat, opens that seat, and has every other seat open its own
 * link in a browser context of its own. Returns the players in seat order, all in the lobby.
 */
export async function createAndJoin(browser: Browser, plans: readonly SeatPlan[]): Promise<Player[]> {
  const [host, ...guests] = plans
  if (host === undefined) throw new Error('a game needs at least one seat')
  const first = await newPlayer(browser, host.name)

  await first.page.goto('/new')
  await first.page.getByRole('radio', { name: /^Normal/ }).check()
  await expect(first.page.getByRole('heading', { name: 'Create a game' })).toBeVisible()
  // A table of any other size than the form's two seats says so first.
  if (plans.length !== 2) await first.page.getByLabel('Seats', { exact: true }).fill(String(plans.length))
  // The earlier way of making a game: every seat gets a name, a deck and a link of its own, and all are held.
  await first.page.getByText('Advanced', { exact: true }).click()
  await first.page.getByRole('checkbox', { name: 'Give every seat its own link' }).check()
  for (const [index, plan] of plans.entries()) {
    await first.page.getByLabel(`Name for seat ${index + 1}`).fill(plan.name)
    await first.page.getByLabel(`Deck for seat ${index + 1}`).selectOption(plan.deck)
  }
  await first.page.getByRole('button', { name: 'Create game' }).click()
  await expect(first.page.getByRole('heading', { name: 'Game created' })).toBeVisible()

  // The links are read from the page, as a host would copy them: `/game/<id>#seat=<n>&token=<token>`.
  const links = first.page.getByRole('list', { name: 'Seat links' }).locator('a[data-share-link]')
  await expect(links).toHaveCount(plans.length)
  const paths = await links.evaluateAll((anchors) =>
    anchors.map((anchor) => anchor.getAttribute('href') ?? ''),
  )

  await first.page.getByRole('button', { name: `Open my seat (${host.name})` }).click()
  const players = [first]
  for (const [index, guest] of guests.entries()) {
    const player = await newPlayer(browser, guest.name)
    const path = paths[index + 1]
    if (path === undefined || !path.startsWith('/game/'))
      throw new Error(`no link for seat ${index + 2}: ${path}`)
    await player.page.goto(path)
    players.push(player)
  }
  for (const { page } of players) {
    await expect(page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
  }
  return players
}

/** The rows of the lobby's seat list, in seat order. */
export const seatRows = (page: Page) => page.getByRole('list', { name: 'Seats' }).getByRole('listitem')

/** What each named seat shows the table of its deck, in seat order: "Deck: Burn, Modern, 3 cards need manual steps". */
export const deckLines = (page: Page) => page.locator('[data-slot="seat-deck"]')

/**
 * Chooses a built-in deck for this page's seat as a player does: the Built-in tab, the deck's row (a click selects it
 * for the preview beside the list), the preview's "Use this deck", then waits for the server's answer. A seat made
 * with no deck (an open seat that was taken) cannot press Ready before this.
 */
export async function chooseDeck(page: Page, deck: { id: string; name: string }): Promise<void> {
  await page.getByRole('tab', { name: 'Built-in' }).click()
  await page.locator(`[data-deck-option="${deck.id}"]`).click()
  await expect(page.getByRole('heading', { level: 3, name: deck.name, exact: true })).toBeVisible()
  const use = page.locator('[data-slot="use-deck"]')
  await expect(use).toBeEnabled()
  await use.click()
  await expect(page.locator('[data-slot="deck-note"]')).toContainText(`Your deck is now ${deck.name}`)
}

/** Presses Ready on every page; the game starts when the last one has. */
export async function everyoneReady(players: readonly Player[]): Promise<void> {
  for (const [index, { page }] of players.entries()) {
    await page.getByRole('button', { name: 'Ready', exact: true }).click()
    // The last Ready starts the game, which can replace the lobby before this line looks at it. The button turns into
    // "Not ready" once the server has said the seat is ready, which is what proves the press went through.
    if (index < players.length - 1) {
      await expect(page.getByRole('button', { name: 'Not ready', exact: true })).toBeVisible()
    }
  }
  for (const { page } of players) {
    await expect(page.getByRole('heading', { name: 'Opening hands' })).toBeVisible()
  }
}
