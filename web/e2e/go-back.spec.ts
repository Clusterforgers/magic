// Goal: against a bot, a seat that passed its precombat main phase gets "Back to Precombat main" at its next prompt, at desktop size and on a phone, and pressing it returns it to that main phase with the control gone and the log saying so; at a table of two people the other is asked first (Allow and Decline), the asker waits on them, an allowed request goes back and a declined one says so and changes nothing.
import { expect, test } from './test'
import type { Browser, Page } from '@playwright/test'
import { logOf } from './manual'
import { WEB_URL } from './ports'

const yourTurn = (page: Page) => page.locator('[data-slot="banner"][data-your-turn]')
const primary = (page: Page) => page.locator('[data-slot="primary-action"]')
const back = (page: Page) => page.getByRole('button', { name: 'Back to Precombat main' })

for (const viewport of [
  { width: 1400, height: 900 },
  { width: 390, height: 844 },
]) {
  test(`a seat goes back to its main phase against a bot at ${viewport.width} px`, async ({ browser }) => {
    test.setTimeout(3 * 60_000)
    const context = await browser.newContext({ viewport })
    const page = await context.newPage()
    const errors: string[] = []
    page.on('pageerror', (error) => errors.push(error.message))
    try {
      await page.goto('/new')
      await page.getByRole('radio', { name: /^Normal/ }).check()
      await page.getByLabel('Bots').selectOption('1')
      await page.getByRole('button', { name: 'Create game' }).click()
      await expect(page.getByRole('heading', { name: 'Waiting for players' })).toBeVisible()
      await page.getByRole('button', { name: 'Ready', exact: true }).click()
      await page.getByRole('button', { name: 'Keep', exact: true }).click()

      // The bot may begin: the seat plays through its turn until its own begins.
      for (let press = 0; press < 60 && (await yourTurn(page).count()) === 0; press++) {
        const end = page.getByRole('button', { name: 'End turn', exact: true })
        if (await end.isVisible()) await end.click()
        else if (await primary(page).isVisible()) await primary(page).click()
        await page.waitForTimeout(300)
      }
      await expect(yourTurn(page)).toBeVisible()
      await expect(primary(page)).toContainText('Next', { timeout: 20_000 })
      await expect(back(page)).toHaveCount(0)

      // It passes its main phase by hand and is asked again later in the turn, where it may go back.
      await primary(page).click()
      await expect(back(page)).toBeVisible({ timeout: 20_000 })
      await expect(back(page)).toBeInViewport()
      await back(page).click()
      await expect(back(page)).toHaveCount(0)
      await logOf(page, [/went back to the precombat main phase/])
      expect(errors, 'page errors').toEqual([])
    } finally {
      await context.close()
    }
  })
}

/** Opens a page for each seat of a new game of two people and brings both past their opening hands. */
async function twoPeople(browser: Browser, viewport: { width: number; height: number }): Promise<Page[]> {
  const made = await fetch(`${WEB_URL}/api/games`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', origin: WEB_URL },
    body: JSON.stringify({
      format: 'normal',
      seats: [
        { name: 'Ana', deck: 'mono-green' },
        { name: 'Bo', deck: 'mono-green' },
      ],
    }),
  })
  expect(made.ok).toBe(true)
  const created = (await made.json()) as { game: string; seats: { seat: number; token: string }[] }
  const pages: Page[] = []
  for (const seat of created.seats) {
    const context = await browser.newContext({ viewport })
    const page = await context.newPage()
    await page.goto(`/game/${created.game}#seat=${seat.seat}&token=${seat.token}`)
    pages.push(page)
  }
  for (const page of pages) await page.getByRole('button', { name: 'Ready', exact: true }).click()
  for (const page of pages) await page.getByRole('button', { name: 'Keep', exact: true }).click()
  return pages
}

/** The asker passes its main phase and the other passes until the asker may go back to it. */
async function passMain(asker: Page, other: Page): Promise<void> {
  await expect(primary(asker)).toContainText('Next', { timeout: 20_000 })
  await primary(asker).click()
  for (let press = 0; press < 40 && !(await back(asker).isVisible()); press++) {
    if (await primary(other).isVisible()) await primary(other).click()
    await asker.waitForTimeout(300)
  }
  await expect(back(asker)).toBeVisible()
}

for (const viewport of [
  { width: 1400, height: 900 },
  { width: 390, height: 844 },
]) {
  test(`at a table of two people the other is asked before going back at ${viewport.width} px`, async ({
    browser,
  }) => {
    test.setTimeout(3 * 60_000)
    const [first, second] = await twoPeople(browser, viewport)
    if (first === undefined || second === undefined) throw new Error('two seats')
    const errors: string[] = []
    for (const page of [first, second]) page.on('pageerror', (error) => errors.push(error.message))
    await expect(yourTurn(first).or(yourTurn(second))).toBeVisible({ timeout: 20_000 })
    const [asker, other] = (await yourTurn(first).count()) > 0 ? [first, second] : [second, first]
    const request = other.getByRole('group', { name: 'Go-back request' })
    const waiting = asker.locator('[data-slot="go-back-waiting"]')

    // Allowed: the asker waits on the other, who allows, and the asker is back in its main phase.
    await passMain(asker, other)
    await back(asker).click()
    await expect(waiting).toContainText(/Waiting for (Ana|Bo)…/)
    await expect(request).toBeVisible()
    await expect(request).toBeInViewport()
    await request.getByRole('button', { name: 'Allow' }).click()
    await expect(request).toHaveCount(0)
    await expect(waiting).toHaveCount(0)
    await logOf(asker, [/went back to the precombat main phase/])

    // Declined: nothing changes and the asker reads why.
    await passMain(asker, other)
    await back(asker).click()
    await request.getByRole('button', { name: 'Decline' }).click()
    await expect(
      asker.getByRole('status').filter({ hasText: /^Declined by (Ana|Bo): (Ana|Bo) does not go back\.$/ }),
    ).toBeVisible()
    await expect(back(asker)).toBeVisible()
    expect(errors, 'page errors').toEqual([])
  })
}
