// Goal: the Rooms scene of the dev table (`/dev/table?prompt=priority-rooms&turn=you`) shows every Room with a badge of two doors (name and lock) that is whole, inside the window and set at 12 px or more, opens a chooser with an "Unlock <door>" line for each locked door of a Room of the viewer inside the window, and previews a Room with both halves and their state, at 1440x900 and 390x844 in the light and the dark theme.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'
import { DEV_WEB_URL } from './ports'
import { expectTheme, themedContext } from './layoutGate'
import { TEXT_FLOOR } from './layoutAudit'
import type { Theme } from './layoutAudit'

const SIZES = [
  { width: 1440, height: 900 },
  { width: 390, height: 844 },
] as const

const THEMES: readonly Theme[] = ['light', 'dark']
const SCENE = `${DEV_WEB_URL}/dev/table?board=duel&prompt=priority-rooms&turn=you`

/** What the page says of the door marks: their words, the size of the text and whether each lies whole in the window. */
async function doorMarks(page: Page) {
  return page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('[data-slot="card-door"]')].map((mark) => {
      const box = mark.getBoundingClientRect()
      const text = mark.querySelector<HTMLElement>('span:not(.sr-only)')
      return {
        unlocked: mark.dataset['unlocked'],
        words: mark.textContent,
        font: Number.parseFloat(getComputedStyle(text ?? mark).fontSize),
        whole: box.width > 0 && box.height > 0 && mark.scrollWidth <= Math.ceil(box.width),
        // Whether a box that clips its contents (the art slot of a face) cuts the mark off.
        clipped: (() => {
          for (let up = mark.parentElement; up !== null; up = up.parentElement) {
            const { overflowX, overflowY } = getComputedStyle(up)
            if (![overflowX, overflowY].some((value) => value === 'hidden' || value === 'clip')) continue
            const around = up.getBoundingClientRect()
            if (
              box.left < around.left - 1 ||
              box.right > around.right + 1 ||
              box.top < around.top - 1 ||
              box.bottom > around.bottom + 1
            ) {
              return true
            }
          }
          return false
        })(),
        inside:
          box.left >= 0 && box.right <= window.innerWidth && box.top >= 0 && box.bottom <= window.innerHeight,
      }
    }),
  )
}

for (const theme of THEMES) {
  for (const size of SIZES) {
    test(`the Rooms scene at ${size.width}x${size.height} in the ${theme} theme`, async ({ browser }) => {
      const context = await themedContext(browser, theme, { viewport: size })
      const page = await context.newPage()
      await page.goto(SCENE)
      await page.locator('[data-region="controls"]').waitFor()
      await expectTheme(page, theme)
      const rooms = page.locator('button[data-object-id]:has([data-slot="card-door"])')
      await expect(rooms).toHaveCount(4)
      // The doors are named by the card's halves once the card endpoint of the scene has answered.
      await expect(page.locator('[data-slot="card-door"]:has-text("Hospital Room")').first()).toBeAttached()

      const marks = await doorMarks(page)
      expect(marks.map((mark) => mark.unlocked).sort()).toEqual([
        'false',
        'false',
        'false',
        'false',
        'true',
        'true',
        'true',
        'true',
      ])
      for (const mark of marks) {
        expect(mark.font, `${mark.words} is set at the floor or above`).toBeGreaterThanOrEqual(TEXT_FLOOR)
        expect(mark.whole, `${mark.words} is whole`).toBe(true)
        // The faces of the narrowest size (40 px wide) have no room for any mark: the doors are in their spoken name and preview.
        if (size.width >= 1024) {
          expect(mark.clipped, `${mark.words} is not cut off by the card`).toBe(false)
        }
        expect(mark.inside, `${mark.words} is inside the window`).toBe(true)
      }

      // The doors are in the spoken name of each Room at every size.
      await expect(rooms.filter({ hasText: 'Bottomless Pool' }).first()).toHaveAttribute(
        'aria-label',
        /Room, Bottomless Pool unlocked, Locker Room locked/,
      )

      // A Room of the viewer's with two locked doors asks which to unlock.
      await rooms.filter({ hasText: 'Surgical Suite' }).click()
      const menu = page.getByRole('menu', { name: /^What to do with / })
      await expect(menu.getByRole('menuitem')).toHaveText([
        /^Unlock Surgical Suite\s*Pay \{1\}\{W\}$/,
        /^Unlock Hospital Room\s*Pay \{3\}\{W\}$/,
      ])
      const box = await menu.boundingBox()
      expect(box, 'the chooser is drawn').not.toBeNull()
      expect(box?.x ?? -1).toBeGreaterThanOrEqual(0)
      expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(size.width)
      expect((box?.y ?? 0) + (box?.height ?? 0)).toBeLessThanOrEqual(size.height)
      await page.keyboard.press('Escape')
      await expect(menu).toBeHidden()

      // The preview lists both halves with their state.
      await rooms.filter({ hasText: 'Bottomless Pool' }).first().hover()
      const preview = page.getByTestId('card-preview')
      await expect(preview.locator('[data-slot="preview-doors"] > li > p:first-child')).toHaveText([
        'Bottomless Pool unlocked',
        'Locker Room locked',
      ])
      await context.close()
    })
  }
}
