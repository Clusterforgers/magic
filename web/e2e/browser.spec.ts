// Goal: a guest finds a public game in the open games list on /, answers the name question in the dialog, lands in the game's lobby on the seat the server gave them (the seat link in the address fragment survives a reload), and the host's lobby shows the new name.
import { expect, test } from '@playwright/test'
import { seatRows } from './lobby'
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
      seats: [
        { name: host, deck: 'mono-green' },
        { name: 'Open seat', deck: 'mono-red' },
      ],
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

    await guestPage.reload()
    await expect(guestPage.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
    await expect(seatRows(guestPage).filter({ hasText: 'Ivy' })).toHaveCount(1)
    expect(errors).toEqual([])
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
