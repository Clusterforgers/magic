// Goal: the host makes a public game on /new and a guest in another browser context joins it from the open games list; the host's toggle takes the game out of the list and back (the guest's lobby follows), and Close game, after its question, sends the host to Play with a notice and shows the guest "The host closed this game."; the host can also remove the guest, who is told so, and a guest who joins again can leave from the lobby and lands on Play.
import { expect, test } from '@playwright/test'
import type { Browser, BrowserContext, Page } from '@playwright/test'
import { deckLines, seatRows } from './lobby'

/** A host name no earlier run used, so a server that is kept between runs lists exactly one such game. */
const unique = (prefix: string): string => `${prefix}${Math.random().toString(36).slice(2, 7)}`

interface Pair {
  host: string
  hostPage: Page
  guestPage: Page
  /** Uncaught exceptions of both pages; the UI should never throw. */
  errors: string[]
  contexts: BrowserContext[]
}

/** A host in the lobby of a new public game, and a second browser context on the Play page. */
async function hostAndGuest(browser: Browser): Promise<Pair> {
  const host = unique('Host')
  const contexts = [await browser.newContext(), await browser.newContext()]
  const [hostPage, guestPage] = await Promise.all(contexts.map((context) => context.newPage()))
  if (hostPage === undefined || guestPage === undefined) throw new Error('no pages')
  const errors: string[] = []
  for (const page of [hostPage, guestPage]) page.on('pageerror', (error) => errors.push(error.message))

  await hostPage.goto('/new')
  await hostPage.getByLabel('Your name').fill(host)
  await hostPage.getByLabel('Your deck').selectOption('mono-green')
  await hostPage.getByRole('button', { name: 'Create game' }).click()
  await expect(hostPage.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
  await guestPage.goto('/')
  return { host, hostPage, guestPage, errors, contexts }
}

/** The guest takes the open seat of `host`'s game from the list, under `name`, and lands in the lobby. */
async function join(guestPage: Page, host: string, name: string): Promise<void> {
  await guestPage.getByRole('button', { name: `Join ${host}'s game, 1 of 2 seats` }).click()
  const dialog = guestPage.getByRole('dialog', { name: `Join ${host}'s game` })
  await dialog.getByLabel('Your name').fill(name)
  await dialog.getByRole('button', { name: 'Join game' }).click()
  await expect(guestPage.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
}

test('the host lists, hides and closes a game, and the guest in another browser sees it end', async ({
  browser,
  request,
}) => {
  const { host, hostPage, guestPage, errors, contexts } = await hostAndGuest(browser)
  /** Who hosts the games the server lists. */
  const listedHosts = async (): Promise<string[]> => {
    const answer = await request.get('/api/lobbies')
    return ((await answer.json()) as { games: { host: string }[] }).games.map((game) => game.host)
  }
  try {
    const listed = hostPage.getByRole('button', { name: 'Listed in the game browser' })
    const inviteOnly = hostPage.getByRole('button', { name: 'Invite only' })
    await expect(listed).toHaveAttribute('aria-pressed', 'true')
    await expect(guestPage.getByRole('button', { name: `Join ${host}'s game, 1 of 2 seats` })).toBeVisible()

    await join(guestPage, host, 'Ivy')
    await expect(seatRows(hostPage).filter({ hasText: 'Ivy' })).toHaveCount(1)
    // The form sent the host's deck and none for the open seat, so the guest chooses theirs in the lobby.
    await expect(deckLines(hostPage).nth(0)).toHaveText('Deck: Mono Green Stompers, Normal, no manual steps')
    await expect(deckLines(hostPage).nth(1)).toHaveText('Deck: no deck yet')
    // The guest sees who may find the game and cannot change it; the host's controls are not on their page.
    await expect(guestPage.locator('[data-slot="lobby-visibility"]')).toContainText(
      'Listed in the game browser',
    )
    await expect(guestPage.getByRole('button', { name: 'Invite only' })).toHaveCount(0)
    await expect(guestPage.getByRole('button', { name: 'Close game' })).toHaveCount(0)
    await expect(guestPage.getByRole('button', { name: 'Leave game' })).toBeVisible()

    // Invite only: the server stops listing the game, and the guest's lobby follows at its next look.
    expect(await listedHosts()).toContain(host)
    await inviteOnly.click()
    await expect(hostPage.getByText('The game is now invite only.')).toBeVisible()
    await expect(inviteOnly).toHaveAttribute('aria-pressed', 'true')
    await expect(guestPage.locator('[data-slot="lobby-visibility"]')).toContainText('Invite only', {
      timeout: 10_000,
    })
    expect(await listedHosts()).not.toContain(host)

    await listed.click()
    await expect(hostPage.getByText('The game is now listed in the game browser.')).toBeVisible()
    expect(await listedHosts()).toContain(host)

    // Close game asks first; Keep game open sends nothing and gives the focus back to the button.
    const close = hostPage.getByRole('button', { name: 'Close game' })
    await close.click()
    const question = hostPage.getByRole('alertdialog', { name: 'Close this game?' })
    await expect(question).toContainText('Everyone in the lobby is sent back to Play.')
    await question.getByRole('button', { name: 'Keep game open' }).click()
    await expect(question).toHaveCount(0)
    await expect(close).toBeFocused()
    await expect(guestPage.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()

    await close.click()
    await question.getByRole('button', { name: 'Close game' }).click()

    // The host is back on Play with a notice; the guest's lobby has become the closed screen.
    await expect(hostPage.getByRole('heading', { name: 'Play', exact: true })).toBeVisible()
    await expect(hostPage.getByRole('alert').filter({ hasText: 'You closed the game.' })).toBeVisible()
    await expect(guestPage.getByRole('heading', { name: 'The host closed this game.' })).toBeVisible()
    await expect(guestPage.getByRole('link', { name: 'Back to Play' })).toBeVisible()
    expect(await listedHosts()).not.toContain(host)
    expect(errors).toEqual([])
  } finally {
    for (const context of contexts) await context.close()
  }
})

test('the host removes a guest, who is told so, and the guest who joins again can leave', async ({
  browser,
}) => {
  const { host, hostPage, guestPage, errors, contexts } = await hostAndGuest(browser)
  try {
    await join(guestPage, host, 'Ivy')
    await expect(seatRows(hostPage).filter({ hasText: 'Ivy' })).toHaveCount(1)

    // Remove names the guest; the guest's page says they were removed, and the seat is open on the host's.
    await hostPage.getByRole('button', { name: 'Remove Ivy from the game' }).click()
    const question = hostPage.getByRole('alertdialog', { name: 'Remove Ivy?' })
    await expect(question).toContainText('Ivy is taken out of seat 2')
    await question.getByRole('button', { name: 'Remove Ivy' }).click()
    await expect(question).toHaveCount(0)
    await expect(hostPage.getByText('Ivy was removed. Seat 2 is open.')).toBeVisible()
    await expect(seatRows(hostPage).filter({ hasText: 'not joined yet' })).toHaveCount(1)
    await expect(guestPage.getByText('You were removed from this game.')).toBeVisible()

    // The seat can be taken again from the list; this time the guest gives it back from the lobby.
    await guestPage.goto('/')
    await join(guestPage, host, 'Ivy')
    await guestPage.getByRole('button', { name: 'Leave game' }).click()
    const leaving = guestPage.getByRole('alertdialog', { name: 'Leave this game?' })
    await expect(leaving).toContainText('Your seat opens for someone else and you go back to Play.')
    await leaving.getByRole('button', { name: 'Leave game' }).click()
    await expect(guestPage.getByRole('heading', { name: 'Play', exact: true })).toBeVisible()
    await expect(guestPage.getByRole('alert').filter({ hasText: 'You left the game.' })).toBeVisible()
    await expect(seatRows(hostPage).filter({ hasText: 'not joined yet' })).toHaveCount(1)
    expect(errors).toEqual([])
  } finally {
    for (const context of contexts) await context.close()
  }
})
