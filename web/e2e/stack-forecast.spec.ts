// Goal: on the dev table (`/dev/table?prompt=priority-forecast`) a Giant Growth on top of the stack shows "+3/+3 → 7/7" in a dark badge on its target, whole, inside the window and at 12 px or more, and on no other card; the badge never covers the target's name, which stays whole and readable, on the duel board and at six seats, at 1440x900, 1366x768 and 390x844.
import { expect, test } from '@playwright/test'
import { DEV_WEB_URL } from './ports'
import { TEXT_FLOOR } from './layoutAudit'

const SIZES = [
  { width: 1440, height: 900 },
  { width: 1366, height: 768 },
  { width: 390, height: 844 },
] as const

const SCENES = {
  duel: `${DEV_WEB_URL}/dev/table?board=duel&prompt=priority-forecast&turn=you`,
  'six seats': `${DEV_WEB_URL}/dev/table?seats=6&prompt=priority-forecast&turn=you`,
} as const

for (const [scene, url] of Object.entries(SCENES)) {
  for (const size of SIZES) {
    test(`the pump is forecast on its target, its name clear (${scene}, ${size.width}x${size.height})`, async ({
      page,
    }) => {
      await page.setViewportSize(size)
      await page.goto(url)
      const badges = page.locator('[data-slot="stack-forecast"][data-place="card"]')
      await expect(badges.first()).toHaveText('+3/+3 → 7/7')
      const marks = await badges.evaluateAll((all) =>
        all.map((badge) => {
          const box = badge.getBoundingClientRect()
          const card = badge.closest<HTMLElement>('[data-object-id]')
          const name = card?.querySelector<HTMLElement>('[data-name]') ?? null
          const line = name?.getBoundingClientRect()
          return {
            words: badge.textContent,
            card: card?.getAttribute('data-object-id') ?? null,
            font: Number.parseFloat(getComputedStyle(badge).fontSize),
            inside: box.left >= 0 && box.top >= 0 && box.right <= innerWidth && box.bottom <= innerHeight,
            whole: badge.scrollWidth <= Math.ceil(box.width),
            named: name !== null && (name.textContent ?? '').trim() !== '',
            // The name line and the badge share no pixel: the badge sits below the name's last line.
            clear: line === undefined || box.top >= line.bottom - 0.5 || box.bottom <= line.top + 0.5,
          }
        }),
      )
      expect(marks.length).toBeGreaterThan(0)
      expect(new Set(marks.map((mark) => mark.card)).size).toBe(1)
      for (const mark of marks) {
        expect(mark.words).toBe('+3/+3 → 7/7')
        expect(mark.font).toBeGreaterThanOrEqual(TEXT_FLOOR)
        expect(mark.named).toBe(true)
        expect(mark.clear).toBe(true)
      }
      // On a phone the board scrolls; at least one drawing of the target shows the badge whole in the window.
      expect(marks.some((mark) => mark.inside && mark.whole)).toBe(true)
    })
  }
}
