// Goal: as a game starts, a seat's page loads the pictures of its own whole deck ahead (cards still in the library included), and neither its picture requests nor its socket frames name a card of the other seat's hidden deck.
import { expect, test } from '@playwright/test'
import type { BrowserContext } from '@playwright/test'
import type { PreconListItem, PreconView } from '../src/protocol/generated'
import { everyoneReady } from './lobby'
import { ACCOUNTS_DATABASE_URL, ACCOUNTS_WEB_URL } from './ports'
import { precons, signUpOnce, startTable } from './table'
import type { Account } from './table'

// Needs Postgres with the card data and the precons (as `precon-game.spec.ts`); skipped without `E2E_DATABASE_URL`.
test.skip(
  ACCOUNTS_DATABASE_URL === undefined,
  'set E2E_DATABASE_URL to a database with cards and precons to run this',
)

// A 1x1 PNG: the pictures come from the page's route, so the test never reaches the CDN.
const PICTURE = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=',
  'base64',
)

/** The ids a deck's cards are pictured by (printing ids where chosen, else oracle ids), apart from its commanders. */
interface DeckIds {
  hidden: Set<string>
  commanders: Set<string>
}

async function deckIds(context: BrowserContext, deck: PreconListItem): Promise<DeckIds> {
  const response = await context.request.get(`${ACCOUNTS_WEB_URL}/api/precons/${deck.code}`)
  expect(response.status()).toBe(200)
  const view = (await response.json()) as PreconView
  const ids: DeckIds = { hidden: new Set(), commanders: new Set() }
  for (const board of view.boards) {
    const into = board.board === 'commander' || board.board === 'companion' ? ids.commanders : ids.hidden
    if (board.board !== 'main' && into === ids.hidden) continue
    for (const card of board.cards) {
      if (card.card === null) continue
      into.add(card.printing_id ?? card.card.oracle_id)
    }
  }
  return ids
}

/** The id a picture URL names: `/api/images/printing/<id>/…` or `/api/images/oracle/<id>/…`. */
const pictured = (url: string): string | null =>
  /\/api\/images\/(?:printing|oracle)\/([^/]+)\//.exec(url)?.[1] ?? null

test.describe('pictures loaded ahead', () => {
  let accounts: Account[] = []
  let chosen: PreconListItem[] = []

  test.beforeAll(async ({ browser }, testInfo) => {
    test.setTimeout(3 * 60_000)
    accounts = await signUpOnce(browser, testInfo, ['pf-ada', 'pf-bo'])
    chosen = (await precons(browser)).verified
  })

  test('a seat loads its own deck ahead and nothing of the hidden deck across the table', async ({
    browser,
  }) => {
    test.setTimeout(3 * 60_000)
    const decks = chosen.slice(0, 2)
    expect(decks, 'the database has fewer than two playable Commander precons').toHaveLength(2)
    const players = await startTable(browser, accounts.slice(0, 2), decks)
    const [host, other] = players
    if (host === undefined || other === undefined) throw new Error('two seats')
    try {
      const [mine, theirs] = await Promise.all(decks.map((deck) => deckIds(host.context, deck)))
      if (mine === undefined || theirs === undefined) throw new Error('two decks')
      // A card in both decks, or the other seat's commander, is no secret of the other deck.
      const secret = [...theirs.hidden].filter((id) => !mine.hidden.has(id) && !mine.commanders.has(id))
      expect(secret.length, 'the other deck shares every card with this one').toBeGreaterThan(0)

      const requested = new Set<string>()
      await host.context.route('**/api/images/**', (route) => {
        const id = pictured(route.request().url())
        if (id !== null) requested.add(id)
        return route.fulfill({ body: PICTURE, contentType: 'image/png' })
      })
      await other.context.route('**/api/images/**', (route) =>
        route.fulfill({ body: PICTURE, contentType: 'image/png' }),
      )
      await everyoneReady(players)
      // Every card of the own deck is asked for while the opening hands are still undecided: most never reached a hand.
      await expect
        .poll(() => [...mine.hidden].filter((id) => !requested.has(id)).length, { timeout: 60_000 })
        .toBe(0)

      // A reconnect is told the list again; its frames are read from the new socket.
      const frames: string[] = []
      host.page.on('websocket', (socket) => {
        socket.on('framereceived', ({ payload }) => {
          if (typeof payload === 'string') frames.push(payload)
        })
      })
      await host.page.reload()
      await expect(host.page.getByRole('heading', { name: 'Opening hands' })).toBeVisible()
      await expect
        .poll(() => frames.some((frame) => frame.includes('"DeckPictures"')), { timeout: 30_000 })
        .toBe(true)
      for (const id of secret) {
        expect(requested.has(id), `the other deck's ${id} was asked for`).toBe(false)
        expect(
          frames.some((frame) => frame.includes(id)),
          `a frame named the other deck's ${id}`,
        ).toBe(false)
      }
      for (const player of players) expect(player.errors, `${player.name}'s page errors`).toEqual([])
    } finally {
      await Promise.all(players.map(({ context }) => context.close()))
    }
  })
})
