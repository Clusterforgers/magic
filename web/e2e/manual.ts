// Goal: the shared steps of the specs that act by hand in the Commands panel: open the side area, open a row, press Apply and read the log.
import { expect } from '@playwright/test'
import type { Locator, Page } from '@playwright/test'

/** The side area: the column at desktop size, the sheet that the top bar's button opens on a phone. */
export async function openSide(page: Page, tab: 'Commands' | 'Log'): Promise<void> {
  const button = page.getByRole('button', { name: /^Commands, log, chat and stack/ })
  if (await button.isVisible()) {
    if ((await button.getAttribute('aria-expanded')) !== 'true') await button.click()
  }
  await page.getByRole('tab', { name: new RegExp(`^${tab}`) }).click()
}

/** The panel's row of a command, opened. */
export async function openRow(page: Page, name: string): Promise<Locator> {
  const panel = page.getByRole('region', { name: 'Commands' })
  const toggle = panel.locator('button[aria-expanded]', { hasText: new RegExp(`^${name}`) }).first()
  // The table redraws while a turn starts: look again until the row is open.
  await expect(async () => {
    if (!(await panel.isVisible())) await openSide(page, 'Commands')
    if ((await toggle.getAttribute('aria-expanded', { timeout: 2000 })) !== 'true')
      await toggle.click({ timeout: 2000 })
    await expect(toggle).toHaveAttribute('aria-expanded', 'true', { timeout: 1000 })
  }).toPass({ timeout: 20_000 })
  const row = panel.locator('[data-open]')
  await expect(row).toHaveCount(1)
  return row
}

/** Presses Apply, which names the change, and waits for the row to say the server took it. */
export async function apply(row: Locator, label: string | RegExp): Promise<void> {
  const button = row.locator('[data-slot="command-apply"]')
  await expect(button).toHaveText(label)
  // The open row is scrolled into the panel's view, so Apply is in reach without a scroll.
  await expect(button).toBeInViewport()
  await button.click()
  await expect(row.locator('[data-slot="command-feedback"]')).toContainText(/^(Done|Sent)/)
}

/** The text of the seat's log, once it holds every line in `lines`. */
export async function logOf(page: Page, lines: readonly RegExp[]): Promise<string> {
  await openSide(page, 'Log')
  const log = page.getByRole('log', { name: 'Game log' })
  for (const line of lines) await expect(log).toContainText(line)
  return (await log.textContent()) ?? ''
}
