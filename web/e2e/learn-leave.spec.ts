// Goal: a learner has one practice game at a time: Leave lesson ends the lesson's game, Restart lesson and a lesson started from the Learn page reached any other way replace the game before, so after any of them only the game on show exists.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { WEB_URL } from './ports'

const coach = (page: Page) => page.locator('[data-slot="coach"]')

/** The game id of the lesson on show. */
function gameOf(page: Page): string {
  const match = /\/game\/([^/#?]+)/.exec(page.url())
  expect(match, page.url()).not.toBeNull()
  return match?.[1] ?? ''
}

/** Starts the course's next lesson from the Learn page and returns its game. */
async function startFromLearn(page: Page): Promise<string> {
  await page.locator('[data-slot="learn-next"]').click()
  await page.waitForURL(/\/game\//)
  await expect(coach(page)).toBeAttached({ timeout: 30_000 })
  return gameOf(page)
}

/** Whether the server still has game `game`. */
async function exists(page: Page, game: string): Promise<boolean> {
  const reply = await page.request.get(`${WEB_URL}/api/games/${game}`)
  expect([200, 404]).toContain(reply.status())
  return reply.status() === 200
}

async function lessonMenu(page: Page, item: string): Promise<void> {
  await page.locator('[data-slot="coach-menu"]').click()
  await page.getByRole('menuitem', { name: item }).click()
}

test('leaving, restarting and starting again leave only the lesson on show', async ({ page }) => {
  test.setTimeout(120_000)
  await page.setViewportSize({ width: 1366, height: 768 })
  await page.goto(`${WEB_URL}/learn`)

  const first = await startFromLearn(page)
  await lessonMenu(page, 'Leave lesson')
  await page.waitForURL(/\/learn$/)
  await expect.poll(() => exists(page, first)).toBe(false)

  const second = await startFromLearn(page)
  expect(second).not.toBe(first)
  await lessonMenu(page, 'Restart lesson')
  await page.waitForURL((url) => !url.pathname.endsWith(second) && url.pathname.startsWith('/game/'))
  await expect(coach(page)).toBeAttached({ timeout: 30_000 })
  const third = gameOf(page)
  expect(await exists(page, second)).toBe(false)
  expect(await exists(page, third)).toBe(true)

  // Going to the Learn page another way is no Leave: the next start replaces the game the tab left behind.
  await page.goto(`${WEB_URL}/learn`)
  await page.waitForURL(/\/learn$/)
  const fourth = await startFromLearn(page)
  expect(await exists(page, third)).toBe(false)
  expect(await exists(page, fourth)).toBe(true)
})
