// Goal: the page an invite link opens takes a guest from the link to the lobby: /join/{code}, typed in lower case with a hyphen as people paste it, shows the code grouped in capitals, asks for a name on the page, opens the seat the server gave in the lobby (the seat link in the address fragment survives a reload) and shows the new name to the host; a code no game has ends in one sentence with the ways to Play and to Create a game, a link that holds no code says so without a request, and both pages fit the window at 1280x720 and 390 wide without scrolling.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { seatRows } from './lobby'
import { WEB_URL } from './ports'

/** A host name no earlier run used, so a server that is kept between runs lists exactly one such game. */
const unique = (prefix: string): string => `${prefix}${Math.random().toString(36).slice(2, 7)}`

/** The two windows the page has to fit: a laptop and a phone. */
const WINDOWS = [
  { width: 1280, height: 720 },
  { width: 390, height: 844 },
]

/** Checks that the page makes no window scroll, at each size, and puts the window back as it was. */
async function fitsEveryWindow(page: Page): Promise<void> {
  const before = page.viewportSize()
  for (const size of WINDOWS) {
    await page.setViewportSize(size)
    const overflow = await page.evaluate(() => ({
      across: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      down: document.documentElement.scrollHeight - document.documentElement.clientHeight,
    }))
    expect(overflow, `${size.width}x${size.height}`).toEqual({ across: 0, down: 0 })
  }
  if (before !== null) await page.setViewportSize(before)
}

test('a guest follows an invite link, gives a name on the page and lands in the lobby', async ({
  browser,
  request,
}) => {
  const host = unique('Host')
  // One held seat (the host's, with a token); the second is open, which is what the link takes.
  const made = await request.post('/api/games', {
    headers: { origin: WEB_URL },
    data: {
      format: 'normal',
      visibility: 'invite',
      held: 1,
      seats: [
        { name: host, deck: 'mono-green' },
        { name: 'Open seat', deck: 'mono-red' },
      ],
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
  const grouped = `${created.code.slice(0, 4)}-${created.code.slice(4)}`

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const errors: string[] = []
  try {
    const hostPage = await hostContext.newPage()
    const guestPage = await guestContext.newPage()
    for (const page of [hostPage, guestPage]) page.on('pageerror', (error) => errors.push(error.message))

    await hostPage.goto(`/game/${created.game}#seat=${hostSeat.seat}&token=${hostSeat.token}`)
    await expect(hostPage.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()

    // As a link survives a chat message: lower case with a hyphen in the middle.
    await guestPage.goto(`/join/${grouped.toLowerCase()}`)
    await expect(guestPage.getByRole('heading', { level: 1, name: 'Join a game' })).toBeVisible()
    await expect(guestPage.getByText(grouped, { exact: true })).toBeVisible()
    const name = guestPage.getByLabel('Your name')
    await expect(name).toBeFocused()
    await fitsEveryWindow(guestPage)

    await name.fill('Ivy')
    await guestPage.getByRole('button', { name: 'Join game' }).click()

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

test('a code no game has ends in one sentence with the ways on, and a link with no code sends nothing', async ({
  page,
}) => {
  const errors: string[] = []
  const joins: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  page.on('request', (request) => {
    if (new URL(request.url()).pathname === '/api/join') joins.push(request.postData() ?? '')
  })
  const main = page.getByRole('main')

  // A code of the right shape that no game holds: the answer is the Play page's sentence, nothing more.
  await page.goto('/join/ABCD2345')
  await expect(page.getByText('ABCD-2345', { exact: true })).toBeVisible()
  await page.getByLabel('Your name').fill('Ivy')
  await page.getByRole('button', { name: 'Join game' }).click()
  await expect(page.getByRole('alert')).toContainText('No open game with that code.')
  await expect(main.getByRole('link', { name: 'Play' })).toBeFocused()
  await expect(main.getByRole('link', { name: 'Create a game' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Join game' })).toHaveCount(0)
  expect(joins).toHaveLength(1)
  await fitsEveryWindow(page)
  await main.getByRole('link', { name: 'Create a game' }).click()
  await expect(page.getByRole('heading', { name: 'Create a game' })).toBeVisible()

  // A link that was cut off holds no code: said at once, with no request.
  await page.goto('/join/ABCD')
  await expect(page.getByRole('alert')).toContainText('That is not a join code')
  await expect(main.getByRole('link', { name: 'Play' })).toBeFocused()
  await fitsEveryWindow(page)
  await main.getByRole('link', { name: 'Play' }).click()
  await expect(page).toHaveURL(/\/$/)
  await expect(page.getByRole('heading', { level: 1, name: 'Play' })).toBeVisible()
  expect(joins).toHaveLength(1)
  expect(errors).toEqual([])
})
