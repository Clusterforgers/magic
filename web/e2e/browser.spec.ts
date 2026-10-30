// Goal: a guest finds a public game in the open games list on /, answers the name question in the dialog, lands in the game's lobby on the seat the server gave them (the seat link in the address fragment survives a reload), and the host's lobby shows the new name; the taken seat has no deck, which both lobbies say, and Ready waits until the guest chooses one there and the host's lobby shows its name, format and manual cards; a guest reaches an invite game, which no list shows, by typing its code, hears one sentence for a code no game has with the typed code kept, and lands in the lobby the answer names.
import { expect, test } from '@playwright/test'
import { chooseDeck, deckLines, seatRows } from './lobby'
import { WEB_URL } from './ports'

/** A host name no earlier run used, so a server that is kept between runs lists exactly one such game. */
const unique = (prefix: string): string => `${prefix}${Math.random().toString(36).slice(2, 7)}`

test('a guest joins a public game from the open games list and lands in its lobby', async ({
  browser,
  request,
}) => {
  const host = unique('Host')
  // One held seat (the host's, with a token); the second is open, which is what the list offers.
  const made = await request.post('/api/games', {
    headers: { origin: WEB_URL },
    data: {
      format: 'normal',
      visibility: 'public',
      held: 1,
      // The open seat names no deck: whoever takes it chooses one in the lobby, as the create form leaves it.
      seats: [{ name: host, deck: 'mono-green' }, { name: 'Open seat' }],
    },
  })
  expect(made.ok()).toBe(true)
  const created = (await made.json()) as { game: string; seats: { seat: number; token: string }[] }
  const hostSeat = created.seats[0]
  if (hostSeat === undefined) throw new Error('the host has no seat')

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const errors: string[] = []
  try {
    const hostPage = await hostContext.newPage()
    const guestPage = await guestContext.newPage()
    for (const page of [hostPage, guestPage]) page.on('pageerror', (error) => errors.push(error.message))

    await hostPage.goto(`/game/${created.game}#seat=${hostSeat.seat}&token=${hostSeat.token}`)
    await expect(hostPage.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()

    await guestPage.goto('/')
    const join = guestPage.getByRole('button', { name: `Join ${host}'s game, 1 of 2 seats` })
    await join.click()
    const dialog = guestPage.getByRole('dialog', { name: `Join ${host}'s game` })
    await expect(dialog.getByLabel('Your name')).toBeFocused()
    await dialog.getByLabel('Your name').fill('Ivy')
    await dialog.getByRole('button', { name: 'Join game' }).click()

    // The token rides in the fragment only: the address is the seat link of seat 2.
    await expect(guestPage).toHaveURL(new RegExp(`/game/${created.game}#seat=1&token=[0-9a-f]{16,}$`))
    await expect(guestPage.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    await expect(seatRows(guestPage).filter({ hasText: 'Ivy' })).toHaveCount(1)
    await expect(seatRows(hostPage).filter({ hasText: 'Ivy' })).toHaveCount(1)

    // The host's seat shows its deck to both lobbies; the taken seat has none, and Ready waits for a choice.
    await expect(deckLines(hostPage).nth(0)).toHaveText(
      'Deck: Mono Green Stompers, Normal, no cards played by hand',
    )
    await expect(deckLines(guestPage).nth(1)).toHaveText('Deck: no deck yet')
    await expect(deckLines(hostPage).nth(1)).toHaveText('Deck: no deck yet')
    const guestReady = guestPage.getByRole('button', { name: 'Ready', exact: true })
    await expect(guestReady).toBeDisabled()
    await expect(guestPage.getByText('Ready needs a deck. Choose one under Your deck.')).toBeVisible()
    await chooseDeck(guestPage, { id: 'mono-red', name: 'Mono Red Giants' })
    await expect(deckLines(hostPage).nth(1)).toHaveText(
      'Deck: Mono Red Giants, Normal, no cards played by hand',
    )
    await expect(guestReady).toBeEnabled()
    await guestReady.click()
    await expect(guestPage.getByRole('button', { name: 'Not ready', exact: true })).toBeVisible()
    await expect(guestPage.getByText(/Your deck is locked while you are ready/)).toBeVisible()
    await guestPage.getByRole('button', { name: 'Not ready', exact: true }).click()

    await guestPage.reload()
    await expect(guestPage.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    await expect(seatRows(guestPage).filter({ hasText: 'Ivy' })).toHaveCount(1)
    expect(errors).toEqual([])
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})

test('a guest joins an invite game with its code, which no list shows, and a wrong code is told in one sentence', async ({
  browser,
  request,
}) => {
  const host = unique('Host')
  const made = await request.post('/api/games', {
    headers: { origin: WEB_URL },
    data: {
      format: 'normal',
      visibility: 'invite',
      held: 1,
      seats: [{ name: host, deck: 'mono-green' }, { name: 'Open seat' }],
    },
  })
  expect(made.ok()).toBe(true)
  const created = (await made.json()) as {
    game: string
    code: string
    seats: { seat: number; token: string }[]
  }
  const hostSeat = created.seats[0]
  if (hostSeat === undefined) throw new Error('the host has no seat')
  // As people type it from a message: lower case with a hyphen in the middle.
  const typed = `${created.code.slice(0, 4)}-${created.code.slice(4)}`.toLowerCase()

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const errors: string[] = []
  try {
    const hostPage = await hostContext.newPage()
    const guestPage = await guestContext.newPage()
    for (const page of [hostPage, guestPage]) page.on('pageerror', (error) => errors.push(error.message))

    await hostPage.goto(`/game/${created.game}#seat=${hostSeat.seat}&token=${hostSeat.token}`)
    await expect(hostPage.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()

    await guestPage.goto('/')
    // The list has been read and holds no row for this game: an invite game is reached by its code alone.
    await expect(guestPage.getByRole('heading', { name: 'Open games', exact: true })).toBeVisible()
    await expect(guestPage.getByText('Loading the open games…')).toHaveCount(0)
    await expect(guestPage.getByRole('button', { name: new RegExp(`^Join ${host}'s game`) })).toHaveCount(0)

    const form = guestPage.getByRole('form', { name: 'Join with a code' })
    const field = form.getByRole('textbox', { name: 'Join with a code' })
    const dialog = guestPage.getByRole('dialog', { name: 'Join with a code' })

    // A code no game has: the dialog asks for the name first, then the answer is one sentence under the field.
    await field.fill('ABCD2345')
    await form.getByRole('button', { name: 'Join', exact: true }).click()
    await expect(dialog.getByLabel('Your name')).toBeFocused()
    await dialog.getByLabel('Your name').fill('Ivy')
    await dialog.getByRole('button', { name: 'Join game' }).click()
    await expect(dialog).toHaveCount(0)
    await expect(form.getByRole('alert')).toHaveText('No open game with that code.')
    await expect(field).toHaveValue('ABCD2345')

    // The right code opens the game on the seat the server gave; the name is remembered from the first try.
    await field.fill(typed)
    await expect(form.getByRole('alert')).toHaveCount(0)
    await form.getByRole('button', { name: 'Join', exact: true }).click()
    await expect(dialog.getByLabel('Your name')).toHaveValue('Ivy')
    await dialog.getByRole('button', { name: 'Join game' }).click()

    await expect(guestPage).toHaveURL(new RegExp(`/game/${created.game}#seat=1&token=[0-9a-f]{16,}$`))
    await expect(guestPage.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    await expect(seatRows(guestPage).filter({ hasText: 'Ivy' })).toHaveCount(1)
    await expect(seatRows(hostPage).filter({ hasText: 'Ivy' })).toHaveCount(1)
    expect(errors).toEqual([])
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
