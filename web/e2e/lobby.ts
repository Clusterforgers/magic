import { expect } from '@playwright/test'
import type { Browser, BrowserContext, Page } from '@playwright/test'

/** One player at the table: their own browser context, so their seat token and storage are not shared. */
export interface Player {
  name: string
  page: Page
  context: BrowserContext
  /** Uncaught exceptions in the page so far; the UI should never throw, so the driver fails on the first. */
  errors: string[]
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

  await first.page.goto('/')
  await expect(first.page.getByRole('heading', { name: 'Create a game' })).toBeVisible()
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

/** Presses Ready on every page; the game starts when the last one has. */
export async function everyoneReady(players: readonly Player[]): Promise<void> {
  for (const [index, { page }] of players.entries()) {
    await page.getByRole('button', { name: 'Ready', exact: true }).click()
    // The last Ready starts the game, which can replace the lobby before this line looks at it.
    if (index < players.length - 1) {
      await expect(page.getByRole('button', { name: 'You are ready' })).toBeVisible()
    }
  }
  for (const { page } of players) {
    await expect(page.getByRole('heading', { name: 'Opening hands' })).toBeVisible()
  }
}
