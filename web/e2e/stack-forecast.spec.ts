// Goal: on the dev table (`/dev/table?prompt=priority-forecast`) a Giant Growth on top of the stack shows "+3/+3 → 7/7" in a dark badge on its target, whole, inside the window and at 12 px or more, and on no other card, at 1440x900 and 390x844.
import { expect, test } from '@playwright/test'
import { DEV_WEB_URL } from './ports'
import { TEXT_FLOOR } from './layoutAudit'

const SIZES = [
  { width: 1440, height: 900 },
  { width: 390, height: 844 },
] as const

const SCENE = `${DEV_WEB_URL}/dev/table?board=duel&prompt=priority-forecast&turn=you`

for (const size of SIZES) {
  test(`the pump on the stack is forecast on its target at ${size.width}x${size.height}`, async ({
    page,
  }) => {
    await page.setViewportSize(size)
    await page.goto(SCENE)
    const badges = page.locator('[data-slot="stack-forecast"][data-place="card"]')
    await expect(badges.first()).toHaveText('+3/+3 → 7/7')
    const marks = await badges.evaluateAll((all) =>
      all.map((badge) => {
        const box = badge.getBoundingClientRect()
        return {
          words: badge.textContent,
          card: badge.closest('[data-object-id]')?.getAttribute('data-object-id') ?? null,
          font: Number.parseFloat(getComputedStyle(badge).fontSize),
          inside: box.left >= 0 && box.top >= 0 && box.right <= innerWidth && box.bottom <= innerHeight,
          whole: badge.scrollWidth <= Math.ceil(box.width),
        }
      }),
    )
    const targets = new Set(marks.map((mark) => mark.card))
    expect(targets.size).toBe(1)
    for (const mark of marks) {
      expect(mark.words).toBe('+3/+3 → 7/7')
      expect(mark.font).toBeGreaterThanOrEqual(TEXT_FLOOR)
    }
    // On a phone the board scrolls; at least one drawing of the target shows the badge whole in the window.
    expect(marks.some((mark) => mark.inside && mark.whole)).toBe(true)
  })
}
